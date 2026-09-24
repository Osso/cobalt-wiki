//! Site member administration: listing members with their emails and roles,
//! changing a member's role, removing and inviting members.
//!
//! Every operation acts for the request's session user, whom Deepwell
//! resolves from the session token header, and requires that user to hold
//! the site's `admin` or `root` role. No parameter names the acting user.

use super::prelude::*;
use crate::models::relation::{self, Entity as Relation};
use crate::models::role::{self, Entity as Role};
use crate::models::user::{self, Entity as User};
use crate::models::user_role::{self, Entity as UserRole};
use crate::services::password_token::{CreatePasswordLink, generate_token};
use crate::services::relation::{
    CreateSiteMember, GetSiteMember, RemoveSiteMember, SiteMemberAccepted, SiteMemberData,
};
use crate::services::role::{GrantUserRoleInput, RevokeUserRoleInput, RoleService};
use crate::services::user::CreateUser;
use crate::services::{PasswordTokenService, RelationService, SiteService, UserService};
use crate::types::{RelationObjectType, RelationType, UserType};
use std::collections::HashMap;
use std::net::IpAddr;
use time::OffsetDateTime;

/// A member's highest site role. `Root` is shown but never assigned here.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Member,
    Moderator,
    Admin,
    Root,
}

impl MemberRole {
    fn role_name(self) -> &'static str {
        match self {
            MemberRole::Member => "member",
            MemberRole::Moderator => "moderator",
            MemberRole::Admin => "admin",
            MemberRole::Root => "root",
        }
    }

    fn from_role_name(name: &str) -> Option<Self> {
        match name {
            "member" => Some(MemberRole::Member),
            "moderator" => Some(MemberRole::Moderator),
            "admin" => Some(MemberRole::Admin),
            "root" => Some(MemberRole::Root),
            _ => None,
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct SiteMemberEntry {
    pub user_id: i64,
    pub name: String,
    pub slug: String,
    pub email: String,
    #[serde(with = "time::serde::rfc3339")]
    pub joined_at: OffsetDateTime,
    pub role: MemberRole,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SetMemberRole {
    pub user_id: i64,
    pub role: MemberRole,
    pub ip_address: IpAddr,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RemoveMember {
    pub user_id: i64,
    pub ip_address: IpAddr,
}

#[derive(Deserialize, Debug, Clone)]
pub struct InviteMember {
    pub email: String,
    /// Account name, required when no account has this email.
    #[serde(default)]
    pub name: Option<String>,
    pub ip_address: IpAddr,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct InviteMemberOutput {
    pub user_id: i64,
    /// Whether the invite created the account.
    pub created: bool,
    /// Whether a set-password link was emailed (new accounts only).
    pub emailed: bool,
}

/// The site admin performing a request.
#[derive(Debug, Copy, Clone)]
struct Actor {
    site_id: i64,
    user_id: i64,
}

/// A member being acted on, with the managed roles they hold.
struct Target {
    user_id: i64,
    held: Vec<(MemberRole, i64)>,
}

#[derive(Debug)]
pub struct MemberAdminService;

impl MemberAdminService {
    /// Whether the user holds the site's `admin` or `root` role.
    pub async fn is_site_admin(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        user_id: i64,
    ) -> Result<bool> {
        let held = Self::held_roles(ctx, site_id, &[user_id]).await?;
        Ok(held.get(&user_id).is_some_and(|roles| {
            roles
                .iter()
                .any(|(role, _)| matches!(role, MemberRole::Admin | MemberRole::Root))
        }))
    }

    pub async fn list(ctx: &ServiceContext<'_>) -> Result<Vec<SiteMemberEntry>> {
        let actor = Self::require_site_admin(ctx).await?;
        let txn = ctx.transaction();
        let memberships = Relation::find()
            .filter(relation::Column::RelationType.eq(RelationType::SiteMember))
            .filter(relation::Column::DestType.eq(RelationObjectType::Site))
            .filter(relation::Column::DestId.eq(actor.site_id))
            .filter(relation::Column::FromType.eq(RelationObjectType::User))
            .filter(relation::Column::OverwrittenAt.is_null())
            .filter(relation::Column::DeletedAt.is_null())
            .order_by_asc(relation::Column::CreatedAt)
            .all(txn)
            .await
            .or_raise(query_error)?;

        let user_ids: Vec<i64> = memberships.iter().map(|m| m.from_id).collect();
        let mut users: HashMap<i64, user::Model> = User::find()
            .filter(user::Column::UserId.is_in(user_ids.clone()))
            .all(txn)
            .await
            .or_raise(query_error)?
            .into_iter()
            .map(|user| (user.user_id, user))
            .collect();
        let held = Self::held_roles(ctx, actor.site_id, &user_ids).await?;

        Ok(memberships
            .into_iter()
            .filter_map(|membership| {
                let user = users.remove(&membership.from_id)?;
                let role = held
                    .get(&user.user_id)
                    .and_then(|roles| roles.iter().map(|(role, _)| *role).max())
                    .unwrap_or(MemberRole::Member);
                Some(SiteMemberEntry {
                    user_id: user.user_id,
                    name: user.name,
                    slug: user.slug,
                    email: user.email,
                    joined_at: membership.created_at,
                    role,
                })
            })
            .collect())
    }

    /// Makes a member a plain member, a moderator or an admin.
    pub async fn set_role(
        ctx: &ServiceContext<'_>,
        SetMemberRole {
            user_id,
            role,
            ip_address,
        }: SetMemberRole,
    ) -> Result<()> {
        if role == MemberRole::Root {
            bail!(Error::new(
                "the root role cannot be assigned",
                ErrorType::BadRequest,
            ));
        }
        let actor = Self::require_site_admin(ctx).await?;
        let target =
            Self::get_changeable_member(ctx, actor, user_id, "change the role of")
                .await?;

        for &(held, role_id) in &target.held {
            if held != MemberRole::Member && held != role {
                Self::revoke(ctx, actor, target.user_id, role_id, ip_address).await?;
            }
        }
        let wanted = if role == MemberRole::Member {
            vec![MemberRole::Member]
        } else {
            vec![MemberRole::Member, role]
        };
        for wanted in wanted {
            if !target.held.iter().any(|(held, _)| *held == wanted) {
                Self::grant(ctx, actor, target.user_id, wanted, ip_address).await?;
            }
        }
        Ok(())
    }

    /// Ends a membership and revokes the member's site roles.
    pub async fn remove(
        ctx: &ServiceContext<'_>,
        RemoveMember {
            user_id,
            ip_address,
        }: RemoveMember,
    ) -> Result<()> {
        let actor = Self::require_site_admin(ctx).await?;
        let target = Self::get_changeable_member(ctx, actor, user_id, "remove").await?;

        for &(_, role_id) in &target.held {
            Self::revoke(ctx, actor, target.user_id, role_id, ip_address).await?;
        }
        RelationService::remove_site_member(
            ctx,
            RemoveSiteMember {
                site_id: actor.site_id,
                user_id: target.user_id,
                removed_by: actor.user_id,
            },
            ip_address,
            "removed by a site admin",
        )
        .await?;
        Ok(())
    }

    /// Adds the account with this email as a member, creating it (and
    /// emailing it a set-password link) when there is none. Any failure,
    /// including the email, undoes the whole invite with the request's
    /// transaction.
    pub async fn invite(
        ctx: &ServiceContext<'_>,
        InviteMember {
            email,
            name,
            ip_address,
        }: InviteMember,
    ) -> Result<InviteMemberOutput> {
        let actor = Self::require_site_admin(ctx).await?;
        let email = email.trim();
        if email.is_empty() {
            bail!(Error::new(
                "email cannot be empty",
                ErrorType::UserEmailEmpty
            ));
        }

        let existing = PasswordTokenService::find_by_email(ctx, email).await?;
        let (user_id, created) = match existing {
            Some(user) => (user.user_id, false),
            None => {
                let name = name.as_deref().map(str::trim).unwrap_or_default();
                if name.is_empty() {
                    bail!(Error::new(
                        "no account has this email, so the invite needs a name",
                        ErrorType::UserNameRequired,
                    ));
                }
                (
                    Self::create_account(ctx, actor, name, email, ip_address).await?,
                    true,
                )
            }
        };

        let membership = GetSiteMember {
            site_id: actor.site_id,
            user_id,
        };
        if RelationService::get_optional_site_member(ctx, membership)
            .await?
            .is_some()
        {
            bail!(Error::new(
                "this account is already a site member",
                ErrorType::SiteMemberExists,
            ));
        }
        RelationService::create_site_member(
            ctx,
            CreateSiteMember {
                site_id: actor.site_id,
                user_id,
                metadata: SiteMemberData {
                    accepted: SiteMemberAccepted::Invitation(actor.user_id),
                },
                created_by: actor.user_id,
            },
            ip_address,
        )
        .await?;
        Self::grant(ctx, actor, user_id, MemberRole::Member, ip_address).await?;

        if created {
            PasswordTokenService::create_link(
                ctx,
                CreatePasswordLink {
                    user: Reference::Id(user_id),
                    site_id: actor.site_id,
                    send_email: true,
                },
            )
            .await?;
        }

        Ok(InviteMemberOutput {
            user_id,
            created,
            emailed: created,
        })
    }

    async fn require_site_admin(ctx: &ServiceContext<'_>) -> Result<Actor> {
        let request = ctx.request();
        let site_id = request.site_id()?;
        let denied = || {
            Error::new(
                "only site admins manage members",
                ErrorType::PermissionDenied,
            )
        };

        let Some(user_id) = request.user_id else {
            bail!(denied());
        };
        // A restricted session has not finished signing in (MFA).
        if request.session.as_ref().is_some_and(|s| s.restricted) {
            bail!(denied());
        }
        if !Self::is_site_admin(ctx, site_id, user_id).await? {
            bail!(denied());
        }
        Ok(Actor { site_id, user_id })
    }

    /// A current member other than the actor who does not hold `root`.
    async fn get_changeable_member(
        ctx: &ServiceContext<'_>,
        actor: Actor,
        user_id: i64,
        action: &str,
    ) -> Result<Target> {
        if user_id == actor.user_id {
            bail!(Error::new(
                format!("you cannot {action} yourself"),
                ErrorType::OwnMembership,
            ));
        }
        RelationService::get_site_member(
            ctx,
            GetSiteMember {
                site_id: actor.site_id,
                user_id,
            },
        )
        .await?;

        let held = Self::held_roles(ctx, actor.site_id, &[user_id])
            .await?
            .remove(&user_id)
            .unwrap_or_default();
        if held.iter().any(|(role, _)| *role == MemberRole::Root) {
            bail!(Error::new(
                format!("you cannot {action} the site's root member"),
                ErrorType::RootMemberProtected,
            ));
        }
        Ok(Target { user_id, held })
    }

    /// Active grants of the managed roles (member, moderator, admin, root)
    /// per user, with their role IDs.
    async fn held_roles(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        user_ids: &[i64],
    ) -> Result<HashMap<i64, Vec<(MemberRole, i64)>>> {
        let txn = ctx.transaction();
        let roles: HashMap<i64, MemberRole> = Role::find()
            .filter(role::Column::SiteId.eq(site_id))
            .filter(role::Column::IsVirtual.eq(false))
            .filter(role::Column::DeletedAt.is_null())
            .all(txn)
            .await
            .or_raise(query_error)?
            .into_iter()
            .filter_map(|role| {
                MemberRole::from_role_name(&role.name)
                    .map(|member| (role.role_id, member))
            })
            .collect();

        let grants = UserRole::find()
            .filter(user_role::Column::SiteId.eq(site_id))
            .filter(user_role::Column::UserId.is_in(user_ids.iter().copied()))
            .filter(user_role::Column::RoleId.is_in(roles.keys().copied()))
            .filter(user_role::Column::DeletedAt.is_null())
            .filter(
                Condition::any()
                    .add(user_role::Column::ExpiresAt.is_null())
                    .add(user_role::Column::ExpiresAt.gt(now())),
            )
            .all(txn)
            .await
            .or_raise(query_error)?;

        let mut held: HashMap<i64, Vec<(MemberRole, i64)>> = HashMap::new();
        for grant in grants {
            held.entry(grant.user_id)
                .or_default()
                .push((roles[&grant.role_id], grant.role_id));
        }
        Ok(held)
    }

    async fn grant(
        ctx: &ServiceContext<'_>,
        actor: Actor,
        user_id: i64,
        role: MemberRole,
        ip_address: IpAddr,
    ) -> Result<()> {
        let role = RoleService::get(
            ctx,
            actor.site_id,
            Reference::Slug(role.role_name().into()),
        )
        .await?;
        RoleService::grant_role_to_user(
            ctx,
            GrantUserRoleInput {
                user_id,
                role_id: role.role_id,
                site_id: actor.site_id,
                assigning_user_id: actor.user_id,
                expires_at: None,
                ip_address,
            },
        )
        .await?;
        Ok(())
    }

    async fn revoke(
        ctx: &ServiceContext<'_>,
        actor: Actor,
        user_id: i64,
        role_id: i64,
        ip_address: IpAddr,
    ) -> Result<()> {
        RoleService::revoke_role_from_user(
            ctx,
            RevokeUserRoleInput {
                user_id,
                role_id,
                site_id: actor.site_id,
                revoking_user_id: actor.user_id,
                ip_address,
            },
        )
        .await?;
        Ok(())
    }

    /// A regular account with a random password nobody is told; its owner
    /// chooses one through the emailed set-password link.
    async fn create_account(
        ctx: &ServiceContext<'_>,
        actor: Actor,
        name: &str,
        email: &str,
        ip_address: IpAddr,
    ) -> Result<i64> {
        let site = SiteService::get(ctx, Reference::Id(actor.site_id)).await?;
        let output = UserService::create(
            ctx,
            CreateUser {
                user_type: UserType::Regular,
                name: name.to_owned(),
                email: email.to_owned(),
                locales: vec![site.locale],
                password: generate_token(),
                bypass_filter: false,
                // The emailed link proves the address; MailCheck is not used.
                bypass_email_verification: true,
                override_user_id: None,
                ip_address,
            },
        )
        .await?;
        Ok(output.user_id)
    }
}

fn query_error() -> Error {
    Error::new("site member query failed", ErrorType::DatabaseQuery)
}
