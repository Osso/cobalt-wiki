//! Watcher preferences, subscriptions, Activity, and token-authorized email opt-out.

use super::prelude::*;
use crate::services::watching::activity::{self, ActivityPage, ChangeDetail};
use crate::services::watching::subscriptions::{
    self, WatchPreferences, WatchScope, WatchSubscription,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct ActivityRequest {
    site_id: i64,
    before_event_id: Option<i64>,
    limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ChangeRequest {
    event_id: i64,
}

#[derive(Deserialize)]
struct UnsubscribeRequest {
    token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnsubscribeOutput {
    unsubscribed: bool,
}

pub async fn watching_activity(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<ActivityPage> {
    let input: ActivityRequest = parse!(params, Request);
    activity::list_activity(
        ctx,
        input.site_id,
        input.before_event_id,
        input.limit.unwrap_or(20),
    )
    .await
}

pub async fn watching_change(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<Option<ChangeDetail>> {
    let input: ChangeRequest = parse!(params, Request);
    activity::get_change(ctx, input.event_id).await
}

pub async fn watching_unsubscribe(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<UnsubscribeOutput> {
    let input: UnsubscribeRequest = parse!(params, Request);
    Ok(UnsubscribeOutput {
        unsubscribed: activity::unsubscribe_email(ctx, &input.token).await?,
    })
}

#[derive(Debug, Deserialize)]
struct ListSubscriptions {
    site_id: i64,
}

#[derive(Debug, Deserialize)]
struct SetSubscription {
    site_id: i64,
    scope: WatchScope,
    target_id: i64,
    watching: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetSubscriptionOutput {
    watching: bool,
}

pub async fn watching_preferences_get(
    ctx: &ServiceContext<'_>,
    _params: Params<'static>,
) -> Result<WatchPreferences> {
    subscriptions::preferences_get(ctx).await
}

pub async fn watching_preferences_set(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<WatchPreferences> {
    let preferences: WatchPreferences = parse!(params, Request);
    subscriptions::preferences_set(ctx, preferences).await
}

pub async fn watching_subscriptions(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<Vec<WatchSubscription>> {
    let ListSubscriptions { site_id } = parse!(params, Request);
    subscriptions::subscriptions(ctx, site_id).await
}

pub async fn watching_subscription_set(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<SetSubscriptionOutput> {
    let SetSubscription {
        site_id,
        scope,
        target_id,
        watching,
    } = parse!(params, Request);
    let watching =
        subscriptions::subscription_set(ctx, site_id, scope, target_id, watching).await?;
    Ok(SetSubscriptionOutput { watching })
}
