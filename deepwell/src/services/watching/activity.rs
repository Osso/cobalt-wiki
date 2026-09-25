//! Recipient-scoped Activity and email opt-out.

use super::visibility::visible_change;
use super::worker::{PendingNotification, actor_name};
use crate::error::prelude::*;
use crate::services::ServiceContext;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use serde::Serialize;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize)]
pub struct ActivityItem {
    pub event_id: i64,
    pub page_id: i64,
    pub revision_id: i64,
    pub previous_revision_id: Option<i64>,
    pub title: String,
    pub slug: String,
    pub actor: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub event_type: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityPage {
    pub items: Vec<ActivityItem>,
    pub next_before: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangeDetail {
    #[serde(flatten)]
    pub item: ActivityItem,
    pub before_text: Option<String>,
    pub after_text: String,
}

pub async fn list_activity(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    before_event_id: Option<i64>,
    limit: u64,
) -> Result<ActivityPage> {
    let user_id = ctx.request().user_id()?;
    if !(1..=50).contains(&limit) || before_event_id.is_some_and(|id| id <= 0) {
        return Err(Error::new(
            "invalid Activity cursor or limit",
            ErrorType::BadRequest,
        )
        .into());
    }
    let rows = ctx.transaction().query_all_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT e.*, n.user_id, n.email_status FROM watch_notification n JOIN watch_event e USING (event_id)
         WHERE n.user_id = $1 AND e.site_id = $2 AND n.activity_ready
           AND ($3::bigint IS NULL OR e.event_id < $3)
         ORDER BY e.event_id DESC LIMIT $4",
        [user_id.into(), site_id.into(), before_event_id.into(), (limit as i64 + 1).into()],
    )).await.or_raise(|| Error::new("failed to load watcher Activity", ErrorType::Page))?;
    let has_more = rows.len() > limit as usize;
    let mut next_before = None;
    let mut items = Vec::new();
    for row in rows.into_iter().take(limit as usize) {
        let notification = PendingNotification::from_query_result(&row, "")
            .or_raise(|| Error::new("invalid Activity event", ErrorType::Page))?;
        next_before = Some(notification.event_id);
        if let Some(change) = load_visible_detail(ctx, &notification).await? {
            items.push(change.item);
        }
    }
    Ok(ActivityPage {
        items,
        next_before: has_more.then_some(next_before).flatten(),
    })
}

pub async fn get_change(
    ctx: &ServiceContext<'_>,
    event_id: i64,
) -> Result<Option<ChangeDetail>> {
    let user_id = ctx.request().user_id()?;
    let row = ctx.transaction().query_one_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT e.*, n.user_id, n.email_status FROM watch_notification n JOIN watch_event e USING (event_id)
         WHERE n.event_id = $1 AND n.user_id = $2 AND n.activity_ready",
        [event_id.into(), user_id.into()],
    )).await.or_raise(|| Error::new("failed to load watcher change", ErrorType::Page))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let notification = PendingNotification::from_query_result(&row, "")
        .or_raise(|| Error::new("invalid watcher change", ErrorType::Page))?;
    load_visible_detail(ctx, &notification).await
}

async fn load_visible_detail(
    ctx: &ServiceContext<'_>,
    notification: &PendingNotification,
) -> Result<Option<ChangeDetail>> {
    let visible = visible_change(
        ctx,
        notification.user_id,
        notification.site_id,
        notification.page_id,
        notification.previous_revision_id,
        notification.new_revision_id,
    )
    .await?;
    let Some(visible) = visible else {
        return Ok(None);
    };
    let actor = actor_name(ctx, notification.actor_user_id).await?;
    Ok(Some(ChangeDetail {
        item: ActivityItem {
            event_id: notification.event_id,
            page_id: notification.page_id,
            revision_id: notification.new_revision_id,
            previous_revision_id: notification.previous_revision_id,
            title: visible.title,
            slug: visible.slug,
            actor,
            created_at: notification.created_at,
            event_type: if notification.previous_revision_id.is_some() {
                "edit"
            } else {
                "create"
            },
        },
        before_text: visible.before,
        after_text: visible.after,
    }))
}

/// A random email-link token authorizes only opting its recipient out of email.
/// Consuming it also revokes that recipient's other old unsubscribe links.
pub async fn unsubscribe_email(ctx: &ServiceContext<'_>, token: &str) -> Result<bool> {
    if uuid::Uuid::parse_str(token).is_err() {
        return Ok(false);
    }
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    let row = ctx.transaction().query_one_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT user_id FROM watch_notification WHERE unsubscribe_token_hash = $1 FOR UPDATE",
        [hash.into()],
    )).await.or_raise(|| Error::new("failed to read email unsubscribe token", ErrorType::User))?;
    let Some(row) = row else {
        return Ok(false);
    };
    let user_id: i64 = row
        .try_get("", "user_id")
        .or_raise(|| Error::new("invalid unsubscribe recipient", ErrorType::User))?;
    ctx.transaction()
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE watch_preferences SET email_enabled = false WHERE user_id = $1",
            [user_id.into()],
        ))
        .await
        .or_raise(|| Error::new("failed to disable watcher email", ErrorType::User))?;
    ctx.transaction().execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "UPDATE watch_notification SET unsubscribe_token_hash = NULL WHERE user_id = $1",
        [user_id.into()],
    )).await.or_raise(|| Error::new("failed to consume watcher unsubscribe links", ErrorType::User))?;
    Ok(true)
}
