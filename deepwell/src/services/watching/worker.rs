//! Deliver committed watcher rows; never replay an ambiguous email attempt.

use super::{
    diff::rendered_diff,
    visibility::{VisibleChange, visible_change},
};
use crate::api::ServerState;
use crate::error::prelude::*;
use crate::models::user;
use crate::services::email::OutgoingEmail;
use crate::services::user::User;
use crate::services::{DomainService, ServiceContext, SiteService, UserService};
use crate::types::Reference;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult, Statement,
    TransactionTrait,
};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use tokio::time::{Duration, sleep};

#[derive(Debug, FromQueryResult)]
pub(crate) struct PendingNotification {
    pub event_id: i64,
    pub user_id: i64,
    pub site_id: i64,
    pub page_id: i64,
    pub previous_revision_id: Option<i64>,
    pub new_revision_id: i64,
    pub actor_user_id: i64,
    pub created_at: OffsetDateTime,
    pub email_status: String,
}

pub async fn run(state: &ServerState) -> Result<()> {
    loop {
        if process_one(state).await? == 0 {
            sleep(Duration::from_secs(1)).await;
        }
    }
}

/// Claim one committed recipient. The claim commits before any external send.
pub async fn process_one(state: &ServerState) -> Result<usize> {
    let transaction =
        state.database.begin().await.or_raise(|| {
            Error::new("failed to begin watcher delivery", ErrorType::Page)
        })?;
    let context = ServiceContext::new(state, &transaction);
    let Some(notification) = load_pending(&context).await? else {
        return Ok(0);
    };
    let email = prepare_delivery(&context, &notification).await?;
    transaction.commit().await.or_raise(|| {
        Error::new("failed to commit watcher delivery claim", ErrorType::Page)
    })?;
    if let Some(email) = email {
        send_claimed_email(state, &notification, email).await?;
    }
    Ok(1)
}

async fn load_pending(ctx: &ServiceContext<'_>) -> Result<Option<PendingNotification>> {
    let row = ctx
        .transaction()
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT e.*, n.user_id, n.email_status FROM watch_notification n
         JOIN watch_event e USING (event_id) WHERE n.processed_at IS NULL
         ORDER BY n.event_id, n.user_id LIMIT 1 FOR UPDATE OF n SKIP LOCKED",
        ))
        .await
        .or_raise(|| {
            Error::new(
                "failed to read pending watcher notification",
                ErrorType::Page,
            )
        })?;
    row.map(|row| {
        PendingNotification::from_query_result(&row, "")
            .or_raise(|| Error::new("invalid watcher notification", ErrorType::Page))
    })
    .transpose()
}

async fn prepare_delivery(
    ctx: &ServiceContext<'_>,
    notification: &PendingNotification,
) -> Result<Option<OutgoingEmail>> {
    let visible = visible_change(
        ctx,
        notification.user_id,
        notification.site_id,
        notification.page_id,
        notification.previous_revision_id,
        notification.new_revision_id,
    )
    .await;
    let visible = match visible {
        Ok(Some(visible)) => visible,
        Ok(None) => {
            finish_claim(ctx, notification, false, "disabled", None, None).await?;
            return Ok(None);
        }
        Err(_) => {
            error!(
                "Watcher revision visibility/rendering failed for event {} recipient {}; no delivery",
                notification.event_id, notification.user_id
            );
            finish_claim(
                ctx,
                notification,
                false,
                "failed",
                None,
                Some("revision visibility/rendering failed"),
            )
            .await?;
            return Ok(None);
        }
    };
    prepare_optional_email(ctx, notification, &visible).await
}

async fn prepare_optional_email(
    ctx: &ServiceContext<'_>,
    notification: &PendingNotification,
    visible: &VisibleChange,
) -> Result<Option<OutgoingEmail>> {
    let recipient = eligible_email_recipient(ctx, notification).await?;
    let Some(recipient) = recipient else {
        finish_claim(ctx, notification, true, "disabled", None, None).await?;
        return Ok(None);
    };
    if ctx.state().mailgun.is_none() {
        error!(
            "Watcher email unavailable for event {}: Mailgun is not configured",
            notification.event_id
        );
        finish_claim(
            ctx,
            notification,
            true,
            "failed",
            None,
            Some("email sender is not configured"),
        )
        .await?;
        return Ok(None);
    }
    let token = uuid::Uuid::new_v4().to_string();
    let token_hash = Sha256::digest(token.as_bytes()).to_vec();
    let email = compose_email(ctx, notification, visible, recipient, &token).await?;
    finish_claim(ctx, notification, true, "attempted", Some(token_hash), None).await?;
    Ok(Some(email))
}

async fn eligible_email_recipient(
    ctx: &ServiceContext<'_>,
    notification: &PendingNotification,
) -> Result<Option<String>> {
    if notification.email_status != "pending" {
        return Ok(None);
    }
    let row = ctx
        .transaction()
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT email_enabled FROM watch_preferences WHERE user_id = $1",
            [notification.user_id.into()],
        ))
        .await
        .or_raise(|| {
            Error::new("failed to read watcher email preference", ErrorType::User)
        })?;
    let enabled = row
        .map(|row| row.try_get::<bool>("", "email_enabled"))
        .transpose()
        .or_raise(|| Error::new("invalid watcher email preference", ErrorType::User))?
        .unwrap_or(false);
    if !enabled {
        return Ok(None);
    }
    let recipient = user::Entity::find_by_id(notification.user_id)
        .one(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new("failed to read watcher email recipient", ErrorType::User)
        })?;
    Ok(recipient
        .filter(|user| user.deleted_at.is_none() && user.email_verified_at.is_some())
        .map(|user| user.email))
}

pub(crate) async fn actor_name(ctx: &ServiceContext<'_>, user_id: i64) -> Result<String> {
    match UserService::get(ctx, Reference::Id(user_id)).await? {
        User::Wikijump(user) => Ok(user.name),
        User::Wikidot(user) => user.name.ok_or_raise(|| {
            Error::new("watcher change author has no display name", ErrorType::User)
        }),
    }
}

async fn compose_email(
    ctx: &ServiceContext<'_>,
    notification: &PendingNotification,
    visible: &VisibleChange,
    recipient: String,
    token: &str,
) -> Result<OutgoingEmail> {
    let site = SiteService::get(ctx, Reference::Id(notification.site_id)).await?;
    let domain = DomainService::preferred_domain(ctx.config(), &site);
    let actor = actor_name(ctx, notification.actor_user_id).await?;
    let event = if notification.previous_revision_id.is_some() {
        "edited"
    } else {
        "created"
    };
    let title = visible.title.replace(['\r', '\n'], " ");
    let diff = rendered_diff(visible.before.as_deref(), &visible.after);
    let change_url = format!(
        "https://{domain}/-/activity?event={}",
        notification.event_id
    );
    let unsubscribe_url =
        format!("https://{domain}/-/watching-unsubscribe?token={token}");
    Ok(OutgoingEmail {
        to: recipient,
        subject: format!("{title} was {event}"),
        text: format!(
            "{title}\n{actor} {event} this page at {}.\n\n{diff}\n\nComplete change: {change_url}\n\nStop watcher email: {unsubscribe_url}\nActivity and watches are not removed by unsubscribing.",
            notification.created_at
        ),
        html: None,
    })
}

async fn finish_claim(
    ctx: &ServiceContext<'_>,
    notification: &PendingNotification,
    ready: bool,
    email_status: &str,
    token_hash: Option<Vec<u8>>,
    failure: Option<&str>,
) -> Result<()> {
    ctx.transaction()
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE watch_notification SET processed_at = now(), activity_ready = $3,
         email_status = $4, unsubscribe_token_hash = $5, last_error = $6
         WHERE event_id = $1 AND user_id = $2",
            [
                notification.event_id.into(),
                notification.user_id.into(),
                ready.into(),
                email_status.into(),
                token_hash.into(),
                failure.into(),
            ],
        ))
        .await
        .or_raise(|| {
            Error::new("failed to record watcher delivery state", ErrorType::Page)
        })?;
    Ok(())
}

async fn send_claimed_email(
    state: &ServerState,
    notification: &PendingNotification,
    email: OutgoingEmail,
) -> Result<()> {
    let sender = state.mailgun.as_ref().ok_or_else(|| {
        Error::new(
            "watcher sender disappeared after claim",
            ErrorType::ConfigSetup,
        )
    })?;
    // Mailgun's sender retries only proven non-acceptance. An ambiguous attempt
    // stays recorded and is never replayed after an acknowledgement loss/crash.
    let sent = sender.send(&email).await.is_ok();
    if !sent {
        error!(
            "Watcher email attempt failed for event {} recipient {}; not replayed",
            notification.event_id, notification.user_id
        );
    }
    state.database.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "UPDATE watch_notification SET email_status = $3, last_error = $4 WHERE event_id = $1 AND user_id = $2",
        [notification.event_id.into(), notification.user_id.into(),
         (if sent { "sent" } else { "failed" }).into(),
         (if sent { None } else { Some("email attempt failed; delivery may be uncertain") }).into()],
    )).await.or_raise(|| Error::new("failed to record watcher email result; attempt must not be replayed", ErrorType::Page))?;
    Ok(())
}
