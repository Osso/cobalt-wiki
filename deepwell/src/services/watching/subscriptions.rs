//! Authenticated local watcher preferences and site/category/page subscriptions.

use crate::error::prelude::*;
use crate::models::user;
use crate::services::ServiceContext;
use crate::services::category::CategoryService;
use crate::services::page::PageService;
use crate::services::permission::{CheckPermissionContext, PermissionService};
use crate::services::site::SiteService;
use crate::types::{Action, Permission, Reference, Resource};
use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, Statement, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchScope {
    Site,
    Category,
    Page,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct WatchPreferences {
    pub email_enabled: bool,
    pub auto_watch: bool,
}

impl Default for WatchPreferences {
    fn default() -> Self {
        Self {
            email_enabled: false,
            auto_watch: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WatchSubscription {
    pub scope: WatchScope,
    pub target_id: i64,
}

fn statement(sql: &str, values: Vec<Value>) -> Statement {
    Statement::from_sql_and_values(DatabaseBackend::Postgres, sql, values)
}

fn denied() -> ExnError {
    Error::new(
        "watch target is not visible in this site",
        ErrorType::PermissionDenied,
    )
    .into()
}

fn request_user(ctx: &ServiceContext<'_>) -> Result<i64> {
    ctx.request().user_id()
}

pub async fn preferences_get(ctx: &ServiceContext<'_>) -> Result<WatchPreferences> {
    let user_id = request_user(ctx)?;
    let row = ctx
        .transaction()
        .query_one_raw(statement(
            "SELECT email_enabled, auto_watch FROM watch_preferences WHERE user_id = $1",
            vec![user_id.into()],
        ))
        .await
        .or_raise(|| Error::new("failed to read watch preferences", ErrorType::User))?;
    match row {
        Some(row) => Ok(WatchPreferences {
            email_enabled: row
                .try_get("", "email_enabled")
                .or_raise(|| Error::new("invalid watch preferences", ErrorType::User))?,
            auto_watch: row
                .try_get("", "auto_watch")
                .or_raise(|| Error::new("invalid watch preferences", ErrorType::User))?,
        }),
        None => Ok(WatchPreferences::default()),
    }
}

pub async fn preferences_set(
    ctx: &ServiceContext<'_>,
    preferences: WatchPreferences,
) -> Result<WatchPreferences> {
    let user_id = request_user(ctx)?;
    if preferences.email_enabled {
        let user = user::Entity::find_by_id(user_id)
            .one(ctx.transaction())
            .await
            .or_raise(|| Error::new("failed to check verified email", ErrorType::User))?;
        if !user.is_some_and(|user| user.email_verified_at.is_some()) {
            return Err(Error::new(
                "verified email required for watcher email",
                ErrorType::BadRequest,
            )
            .into());
        }
    }
    ctx.transaction().execute_raw(statement(
        "INSERT INTO watch_preferences (user_id, email_enabled, auto_watch) VALUES ($1, $2, $3)
         ON CONFLICT (user_id) DO UPDATE SET email_enabled = EXCLUDED.email_enabled, auto_watch = EXCLUDED.auto_watch",
        vec![user_id.into(), preferences.email_enabled.into(), preferences.auto_watch.into()],
    )).await.or_raise(|| Error::new("failed to save watch preferences", ErrorType::User))?;
    Ok(preferences)
}

async fn can_view_site(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    user_id: i64,
) -> Result<bool> {
    if SiteService::get_optional(ctx, Reference::Id(site_id))
        .await?
        .is_none()
    {
        return Ok(false);
    }
    PermissionService::check_user_can(
        ctx,
        &CheckPermissionContext {
            user_id: Some(user_id),
            site_id,
            page_reference: None,
        },
        Permission {
            resource_type: Resource::Site,
            resource_category: None,
            action: Action::View,
        },
    )
    .await
}

async fn can_view_target(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    user_id: i64,
    scope: WatchScope,
    target_id: i64,
) -> Result<bool> {
    if !can_view_site(ctx, site_id, user_id).await? {
        return Ok(false);
    }
    let (category_id, page_reference) = match scope {
        WatchScope::Site => return Ok(target_id == site_id),
        WatchScope::Category => {
            if CategoryService::get_optional(ctx, site_id, Reference::Id(target_id))
                .await?
                .is_none()
            {
                return Ok(false);
            }
            (target_id, None)
        }
        WatchScope::Page => {
            let Some(page) =
                PageService::get_direct_optional(ctx, target_id, false).await?
            else {
                return Ok(false);
            };
            if page.site_id != site_id {
                return Ok(false);
            }
            (page.page_category_id, Some(Reference::Id(page.page_id)))
        }
    };
    PermissionService::check_user_can(
        ctx,
        &CheckPermissionContext {
            user_id: Some(user_id),
            site_id,
            page_reference,
        },
        Permission {
            resource_type: Resource::Page,
            resource_category: Some(Reference::Id(category_id)),
            action: Action::View,
        },
    )
    .await
}

fn scope_columns(scope: WatchScope, target_id: i64) -> (Option<i64>, Option<i64>) {
    match scope {
        WatchScope::Site => (None, None),
        WatchScope::Category => (Some(target_id), None),
        WatchScope::Page => (None, Some(target_id)),
    }
}

pub async fn subscriptions(
    ctx: &ServiceContext<'_>,
    site_id: i64,
) -> Result<Vec<WatchSubscription>> {
    let user_id = request_user(ctx)?;
    if !can_view_site(ctx, site_id, user_id).await? {
        return Ok(Vec::new());
    }
    let rows = ctx.transaction().query_all_raw(statement(
        "SELECT category_id, page_id FROM watch_subscription WHERE user_id = $1 AND site_id = $2 ORDER BY subscription_id",
        vec![user_id.into(), site_id.into()],
    )).await.or_raise(|| Error::new("failed to list watch subscriptions", ErrorType::User))?;
    let mut visible = Vec::new();
    for row in rows {
        let category_id: Option<i64> = row
            .try_get("", "category_id")
            .or_raise(|| Error::new("invalid watch subscription", ErrorType::User))?;
        let page_id: Option<i64> = row
            .try_get("", "page_id")
            .or_raise(|| Error::new("invalid watch subscription", ErrorType::User))?;
        let subscription = match (category_id, page_id) {
            (Some(target_id), None) => WatchSubscription {
                scope: WatchScope::Category,
                target_id,
            },
            (None, Some(target_id)) => WatchSubscription {
                scope: WatchScope::Page,
                target_id,
            },
            (None, None) => WatchSubscription {
                scope: WatchScope::Site,
                target_id: site_id,
            },
            (Some(_), Some(_)) => {
                return Err(Error::new(
                    "invalid watch subscription scope",
                    ErrorType::User,
                )
                .into());
            }
        };
        if can_view_target(
            ctx,
            site_id,
            user_id,
            subscription.scope,
            subscription.target_id,
        )
        .await?
        {
            visible.push(subscription);
        }
    }
    Ok(visible)
}

pub async fn subscription_set(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    scope: WatchScope,
    target_id: i64,
    watching: bool,
) -> Result<bool> {
    let user_id = request_user(ctx)?;
    if watching && !can_view_target(ctx, site_id, user_id, scope, target_id).await? {
        return Err(denied());
    }
    if matches!(scope, WatchScope::Site) && target_id != site_id {
        return Err(denied());
    }
    let (category_id, page_id) = scope_columns(scope, target_id);
    let values = vec![
        user_id.into(),
        site_id.into(),
        category_id.into(),
        page_id.into(),
    ];
    let sql = if watching {
        "INSERT INTO watch_subscription (user_id, site_id, category_id, page_id)
         VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING"
    } else {
        "DELETE FROM watch_subscription WHERE user_id = $1 AND site_id = $2
         AND category_id IS NOT DISTINCT FROM $3 AND page_id IS NOT DISTINCT FROM $4"
    };
    ctx.transaction()
        .execute_raw(statement(sql, values))
        .await
        .or_raise(|| {
            Error::new("failed to update watch subscription", ErrorType::User)
        })?;
    Ok(watching)
}
