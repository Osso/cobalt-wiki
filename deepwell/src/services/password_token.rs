//! Single-use links that let a user choose a password: member invites made
//! by a trusted caller, and forgotten-password requests by email address.
//!
//! The token only ever appears in the link. The database keeps its SHA-256,
//! so a database read cannot be turned into a working link.

use super::prelude::*;
use crate::models::password_token::{self, Entity as PasswordToken};
use crate::models::site::Model as SiteModel;
use crate::models::user::{self, Entity as User, Model as UserModel};
use crate::services::email::{MailgunSender, OutgoingEmail};
use crate::services::user::UpdateUserBody;
use crate::services::{DomainService, SiteService, UserService};
use crate::types::UserType;
use crate::utils::assert_is_csprng;
use data_encoding::BASE64URL_NOPAD;
use rand::Rng;
use rand::RngExt;
use sea_orm::UpdateResult;
use sea_query::{Expr, Func};
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use time::{Duration, OffsetDateTime};

pub const INVITE_LIFETIME: Duration = Duration::days(7);
pub const RESET_LIFETIME: Duration = Duration::hours(24);

/// A forgotten-password request this soon after the user's last link sends
/// nothing, so nobody can flood a member's inbox.
const RESET_COOLDOWN: Duration = Duration::minutes(10);

#[derive(Deserialize, Debug, Clone)]
pub struct CreatePasswordLink<'a> {
    pub user: Reference<'a>,
    pub site_id: i64,
    #[serde(default)]
    pub send_email: bool,
}

#[derive(Serialize, Debug, Clone)]
pub struct CreatePasswordLinkOutput {
    pub path: String,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    pub emailed: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RedeemPasswordToken {
    pub token: String,
    pub password: String,
    pub ip_address: IpAddr,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RequestPasswordReset {
    pub email: String,
    pub site_id: i64,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct PasswordLinkStatus {
    /// Whether the user ever redeemed a link, that is chose their own password.
    pub password_chosen: bool,
    pub pending: Option<PendingPasswordLink>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct PendingPasswordLink {
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    pub emailed: bool,
}

#[derive(Debug)]
pub struct PasswordTokenService;

impl PasswordTokenService {
    /// For trusted callers only: returns a working link to someone's account.
    pub async fn create_link(
        ctx: &ServiceContext<'_>,
        CreatePasswordLink {
            user,
            site_id,
            send_email,
        }: CreatePasswordLink<'_>,
    ) -> Result<CreatePasswordLinkOutput> {
        let user = Self::get_regular_user(ctx, user).await?;
        let site = SiteService::get(ctx, Reference::Id(site_id)).await?;

        // Checked before anything is written, so a failed send leaves no link.
        let sender = if send_email {
            if !is_deliverable(&user.email) {
                bail!(Error::new(
                    format!("user {} has no deliverable email address", user.slug),
                    ErrorType::BadRequest,
                ));
            }
            Some(mailgun(ctx)?)
        } else {
            None
        };

        let (token, expires_at) =
            Self::issue(ctx, user.user_id, INVITE_LIFETIME, send_email).await?;
        if let Some(sender) = sender {
            let url = link_url(ctx.config(), &site, &token);
            sender
                .send(&invite_email(&user, &site, &url, expires_at))
                .await?;
        }

        Ok(CreatePasswordLinkOutput {
            path: link_path(&token),
            expires_at,
            emailed: send_email,
        })
    }

    pub async fn status(
        ctx: &ServiceContext<'_>,
        user: Reference<'_>,
    ) -> Result<PasswordLinkStatus> {
        let user = Self::get_regular_user(ctx, user).await?;
        let tokens = PasswordToken::find()
            .filter(password_token::Column::UserId.eq(user.user_id))
            .all(ctx.transaction())
            .await
            .or_raise(query_error)?;
        let now = now();
        Ok(PasswordLinkStatus {
            password_chosen: tokens.iter().any(|token| token.used_at.is_some()),
            pending: tokens
                .iter()
                .find(|token| token.used_at.is_none() && token.expires_at > now)
                .map(|token| PendingPasswordLink {
                    expires_at: token.expires_at,
                    emailed: token.emailed_at.is_some(),
                }),
        })
    }

    /// Sets the password of the link's user and uses up the link. Unknown,
    /// used and expired links fail with distinct errors that name no user.
    pub async fn redeem(
        ctx: &ServiceContext<'_>,
        RedeemPasswordToken {
            token,
            password,
            ip_address,
        }: RedeemPasswordToken,
    ) -> Result<()> {
        if password.is_empty() {
            bail!(Error::new(
                "password cannot be empty",
                ErrorType::EmptyPassword,
            ));
        }

        let txn = ctx.transaction();
        let row = PasswordToken::find()
            .filter(password_token::Column::TokenHash.eq(hash_token(&token)))
            .one(txn)
            .await
            .or_raise(query_error)?
            .ok_or_else(|| {
                Error::new("unknown password link", ErrorType::PasswordTokenInvalid)
            })?;
        if row.used_at.is_some() {
            bail!(Error::new(
                "password link already used",
                ErrorType::PasswordTokenUsed,
            ));
        }
        let now = now();
        if row.expires_at <= now {
            bail!(Error::new(
                "password link expired",
                ErrorType::PasswordTokenExpired,
            ));
        }

        // A concurrent redemption of the same link waits on this row, then
        // matches nothing.
        let UpdateResult { rows_affected, .. } = PasswordToken::update_many()
            .col_expr(password_token::Column::UsedAt, Expr::value(now))
            .filter(password_token::Column::TokenId.eq(row.token_id))
            .filter(password_token::Column::UsedAt.is_null())
            .exec(txn)
            .await
            .or_raise(query_error)?;
        if rows_affected != 1 {
            bail!(Error::new(
                "password link already used",
                ErrorType::PasswordTokenUsed,
            ));
        }

        UserService::update(
            ctx,
            Reference::Id(row.user_id),
            ip_address,
            UpdateUserBody {
                password: Maybe::Set(password),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }

    /// Emails a link when the address belongs to a regular user. The caller
    /// gets the same answer, at about the same time, either way.
    pub async fn request_reset(
        ctx: &ServiceContext<'_>,
        RequestPasswordReset { email, site_id }: RequestPasswordReset,
    ) -> Result<()> {
        // Before the lookup, so an unconfigured server fails for every address.
        let sender = mailgun(ctx)?;
        let site = SiteService::get(ctx, Reference::Id(site_id)).await?;
        let email = email.trim();
        if email.is_empty() {
            bail!(Error::new("email cannot be empty", ErrorType::BadRequest));
        }

        let Some(user) = Self::find_by_email(ctx, email).await? else {
            info!("Password reset requested for an address with no account");
            return Ok(());
        };
        if Self::issued_since(ctx, user.user_id, now() - RESET_COOLDOWN).await? {
            info!(
                "Password reset for user ID {} skipped, a link was sent recently",
                user.user_id,
            );
            return Ok(());
        }

        let (token, _) = Self::issue(ctx, user.user_id, RESET_LIFETIME, true).await?;
        let message = reset_email(&user, &site, &link_url(ctx.config(), &site, &token));

        // Waiting for Mailgun, or returning its failure, would tell the caller
        // that the address has an account.
        tokio::spawn(async move {
            if let Err(error) = sender.send(&message).await {
                error!("Failed to send password reset email: {error}");
            }
        });
        Ok(())
    }

    /// Replaces the user's unused links with a new one: only the newest works.
    async fn issue(
        ctx: &ServiceContext<'_>,
        user_id: i64,
        lifetime: Duration,
        emailed: bool,
    ) -> Result<(String, OffsetDateTime)> {
        let txn = ctx.transaction();
        PasswordToken::delete_many()
            .filter(password_token::Column::UserId.eq(user_id))
            .filter(password_token::Column::UsedAt.is_null())
            .exec(txn)
            .await
            .or_raise(query_error)?;

        let token = generate_token();
        let created_at = now();
        let expires_at = created_at + lifetime;
        password_token::ActiveModel {
            user_id: Set(user_id),
            token_hash: Set(hash_token(&token)),
            created_at: Set(created_at),
            expires_at: Set(expires_at),
            emailed_at: Set(emailed.then_some(created_at)),
            ..Default::default()
        }
        .insert(txn)
        .await
        .or_raise(query_error)?;
        Ok((token, expires_at))
    }

    async fn issued_since(
        ctx: &ServiceContext<'_>,
        user_id: i64,
        since: OffsetDateTime,
    ) -> Result<bool> {
        let count = PasswordToken::find()
            .filter(password_token::Column::UserId.eq(user_id))
            .filter(password_token::Column::CreatedAt.gt(since))
            .count(ctx.transaction())
            .await
            .or_raise(query_error)?;
        Ok(count > 0)
    }

    pub(crate) async fn find_by_email(
        ctx: &ServiceContext<'_>,
        email: &str,
    ) -> Result<Option<UserModel>> {
        User::find()
            .filter(
                Expr::expr(Func::lower(Expr::col(user::Column::Email)))
                    .eq(email.to_lowercase()),
            )
            .filter(user::Column::UserType.eq(UserType::Regular))
            .filter(user::Column::DeletedAt.is_null())
            .one(ctx.transaction())
            .await
            .or_raise(query_error)
    }

    async fn get_regular_user(
        ctx: &ServiceContext<'_>,
        reference: Reference<'_>,
    ) -> Result<UserModel> {
        let user = UserService::get_real(ctx, reference).await?;
        if user.user_type != UserType::Regular || user.deleted_at.is_some() {
            bail!(Error::new(
                format!("user {} cannot sign in with a password", user.slug),
                ErrorType::UserWrongType,
            ));
        }
        Ok(user)
    }
}

fn mailgun(ctx: &ServiceContext<'_>) -> Result<MailgunSender> {
    ctx.state().mailgun.clone().ok_or_else(|| {
        Error::new(
            "email sending is not configured: set MAILGUN_API_KEY, MAILGUN_DOMAIN and MAILGUN_FROM",
            ErrorType::EmailSend,
        )
        .into()
    })
}

fn query_error() -> Error {
    Error::new("password link query failed", ErrorType::DatabaseQuery)
}

pub(crate) fn generate_token() -> String {
    let mut rng = rand::rng();
    assert_is_csprng(&rng);
    let mut buffer = [0; 32];
    rng.fill(&mut buffer);
    BASE64URL_NOPAD.encode(&buffer)
}

fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn link_path(token: &str) -> String {
    format!("/-/set-password/{token}")
}

fn link_url(config: &Config, site: &SiteModel, token: &str) -> String {
    let domain = DomainService::preferred_domain(config, site);
    format!("https://{domain}{}", link_path(token))
}

/// Imported members start with an RFC 2606 `.invalid` placeholder address.
fn is_deliverable(email: &str) -> bool {
    email.contains('@') && !email.to_ascii_lowercase().ends_with(".invalid")
}

fn invite_email(
    user: &UserModel,
    site: &SiteModel,
    url: &str,
    expires_at: OffsetDateTime,
) -> OutgoingEmail {
    OutgoingEmail {
        to: user.email.clone(),
        subject: format!("Choose your password for {}", site.name),
        text: format!(
            "Hello {name},\n\n\
             Your account {name} on {site} is ready. To sign in, first choose \
             a password here:\n\n{url}\n\n\
             The link works once and expires on {date} (UTC).\n",
            name = user.name,
            site = site.name,
            date = expires_at.date(),
        ),
        html: None,
    }
}

fn reset_email(user: &UserModel, site: &SiteModel, url: &str) -> OutgoingEmail {
    OutgoingEmail {
        to: user.email.clone(),
        subject: format!("Reset your password for {}", site.name),
        text: format!(
            "Hello {name},\n\n\
             Someone asked to reset the password of your account {name} on \
             {site}. To choose a new password, open:\n\n{url}\n\n\
             The link works once and expires in 24 hours. If you did not ask \
             for this, ignore this email: your password stays the same.\n",
            name = user.name,
            site = site.name,
        ),
        html: None,
    }
}

#[test]
fn tokens_are_random_url_safe_and_stored_hashed() {
    let token = generate_token();
    assert_eq!(token.len(), 43);
    assert!(
        token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    );
    assert_ne!(token, generate_token());
    assert_eq!(hash_token(&token), hash_token(&token));
    assert_ne!(hash_token(&token), token.as_bytes());
    assert_eq!(hash_token(&token).len(), 32);
}

#[test]
fn placeholder_addresses_are_not_deliverable() {
    assert!(!is_deliverable("wikidot-7444794@members.invalid"));
    assert!(!is_deliverable("WIKIDOT-1@MEMBERS.INVALID"));
    assert!(!is_deliverable(""));
    assert!(is_deliverable("alice@example.com"));
}
