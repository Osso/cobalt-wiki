import { client } from "$lib/server/deepwell"
import type { RequestContext } from "$lib/server/load/request-ctx"

export type WatchScope = "site" | "category" | "page"
export interface WatchSubscription {
  scope: WatchScope
  target_id: number
}
export interface WatchingPreferences {
  email_enabled: boolean
  auto_watch: boolean
}
export interface WatchingEvent {
  event_id: number
  page_id: number
  revision_id: number
  previous_revision_id: number | null
  title: string
  slug: string
  actor: string
  created_at: string
  event_type: "create" | "edit"
}
export interface WatchingActivity {
  items: WatchingEvent[]
  next_before: number | null
}

export interface WatchingChange extends WatchingEvent {
  before_text: string | null
  after_text: string
}

export function watchingPreferencesGet(
  context: RequestContext
): Promise<WatchingPreferences> {
  return client.request("watching_preferences_get", {}, context)
}

export function watchingPreferencesSet(
  preferences: WatchingPreferences,
  context: RequestContext
): Promise<WatchingPreferences> {
  return client.request("watching_preferences_set", preferences, context)
}

export function watchingSubscriptions(
  siteId: number,
  context: RequestContext
): Promise<WatchSubscription[]> {
  return client.request("watching_subscriptions", { site_id: siteId }, context)
}

export function watchingSubscriptionSet(
  siteId: number,
  scope: WatchScope,
  targetId: number,
  watching: boolean,
  context: RequestContext
): Promise<{ watching: boolean }> {
  return client.request(
    "watching_subscription_set",
    { site_id: siteId, scope, target_id: targetId, watching },
    context
  )
}

export function watchingActivity(
  siteId: number,
  beforeEventId: number | undefined,
  limit: number,
  context: RequestContext
): Promise<WatchingActivity> {
  return client.request(
    "watching_activity",
    {
      site_id: siteId,
      ...(beforeEventId === undefined ? {} : { before_event_id: beforeEventId }),
      limit
    },
    context
  )
}

export function watchingChange(
  eventId: number,
  context: RequestContext
): Promise<WatchingChange | null> {
  return client.request("watching_change", { event_id: eventId }, context)
}

export function watchingUnsubscribe(token: string): Promise<{ unsubscribed: boolean }> {
  return client.request("watching_unsubscribe", { token })
}
