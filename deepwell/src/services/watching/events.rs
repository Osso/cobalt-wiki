//! Transactional watcher events for the ordinary page create/edit endpoints.

use crate::error::prelude::*;
use crate::services::permission::CheckPermissionContext;
use crate::services::{PageRevisionService, PageService, ServiceContext};
use crate::types::{Action, Reference};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

/// Persist the change and its subscription recipients in the page transaction.
/// Import and revision-replay endpoints deliberately do not call this function.
pub async fn record_change(
    ctx: &ServiceContext<'_>,
    previous_revision_id: Option<i64>,
    new_revision_id: i64,
    suppress: bool,
) -> Result<()> {
    let revision = PageRevisionService::get_direct(ctx, new_revision_id).await?;
    let page =
        PageService::get(ctx, revision.site_id, Reference::Id(revision.page_id)).await?;
    if let Some(previous_id) = previous_revision_id {
        verify_previous_page(ctx, previous_id, page.site_id, page.page_id).await?;
        auto_watch_edited_page(ctx, revision.user_id, page.site_id, page.page_id).await?;
    }
    if suppress {
        return Ok(());
    }
    let event = ctx.transaction().query_one_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "INSERT INTO watch_event (site_id, page_id, previous_revision_id, new_revision_id, actor_user_id)
         VALUES ($1, $2, $3, $4, $5) ON CONFLICT (new_revision_id) DO NOTHING RETURNING event_id",
        [page.site_id.into(), page.page_id.into(), previous_revision_id.into(),
         new_revision_id.into(), revision.user_id.into()],
    )).await.or_raise(|| Error::new("failed to record watcher change", ErrorType::Page))?;
    let Some(event) = event else {
        return Ok(());
    };
    let event_id: i64 = event
        .try_get("", "event_id")
        .or_raise(|| Error::new("invalid watcher event identity", ErrorType::Page))?;
    enqueue_recipients(
        ctx,
        event_id,
        page.site_id,
        revision.user_id,
        page.page_category_id,
        page.page_id,
    )
    .await?;
    Ok(())
}

async fn verify_previous_page(
    ctx: &ServiceContext<'_>,
    previous_id: i64,
    site_id: i64,
    page_id: i64,
) -> Result<()> {
    let previous = PageRevisionService::get_direct(ctx, previous_id).await?;
    if previous.page_id != page_id || previous.site_id != site_id {
        return Err(Error::new(
            "watch revisions belong to different pages",
            ErrorType::Page,
        )
        .into());
    }
    Ok(())
}

async fn enqueue_recipients(
    ctx: &ServiceContext<'_>,
    event_id: i64,
    site_id: i64,
    actor_id: i64,
    category_id: i64,
    page_id: i64,
) -> Result<()> {
    ctx.transaction().execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "INSERT INTO watch_notification (event_id, user_id, email_status)
         SELECT $1, s.user_id, CASE WHEN coalesce(p.email_enabled, false) THEN 'pending' ELSE 'disabled' END
         FROM watch_subscription s LEFT JOIN watch_preferences p ON p.user_id = s.user_id
         WHERE s.site_id = $2 AND s.user_id <> $3
           AND ((s.category_id IS NULL AND s.page_id IS NULL) OR s.category_id = $4 OR s.page_id = $5)
         GROUP BY s.user_id, p.email_enabled ON CONFLICT (event_id, user_id) DO NOTHING",
        [event_id.into(), site_id.into(), actor_id.into(),
         category_id.into(), page_id.into()],
    )).await.or_raise(|| Error::new("failed to record watcher recipients", ErrorType::Page))?;
    Ok(())
}

async fn auto_watch_edited_page(
    ctx: &ServiceContext<'_>,
    user_id: i64,
    site_id: i64,
    page_id: i64,
) -> Result<()> {
    let permission = CheckPermissionContext {
        user_id: Some(user_id),
        site_id,
        page_reference: Some(Reference::Id(page_id)),
    };
    if !PageService::check_user_permission(ctx, &permission, Action::View).await? {
        return Ok(());
    }
    ctx.transaction()
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO watch_subscription (user_id, site_id, page_id)
         SELECT user_id, $2, $3 FROM watch_preferences WHERE user_id = $1 AND auto_watch
         ON CONFLICT (user_id, site_id, page_id) WHERE page_id IS NOT NULL DO NOTHING",
            [user_id.into(), site_id.into(), page_id.into()],
        ))
        .await
        .or_raise(|| Error::new("failed to watch edited page", ErrorType::Page))?;
    Ok(())
}
