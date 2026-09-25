# Cobalt watching

Cobalt will support Wikidot-style watching for locally created page subscriptions. This specification defines the required behavior; the current implementation state is described in the [replica status](../wiki/systems/cobalt-replica-status.md). Its source basis is Wikidot's [watching FAQ](https://www.wikidot.com/faq:watching), which covers edit/comment watches, preferences, unsubscribing, and unique email links, plus the historical [staff explanation](https://community.wikidot.com/forum/t-456532/what-s-going-on-with-watching) of Activity with optional email. Comments remain deferred here.

## What it must do

### Page subscriptions

- [ ] A signed-in user may subscribe to and unsubscribe from a page they can read. The subscription uses the existing `PageWatch` user-to-page relation.
- [ ] Subscribing, viewing Activity, and receiving email each require current read visibility for the page. This privacy requirement must not assume or define global role policy.
- [ ] New local subscriptions only: imported or migrated users are not auto-enrolled, and migration tooling does not import subscriptions.
- [ ] No automatic watches exist by default. A user may opt in to automatically watch pages after editing them.
- [ ] The supported initial event scope is page creation and page edits. Later edits to imported pages are ordinary future events and may notify locally subscribed users.

### Activity and email

- [ ] Page changes appear in each eligible subscriber's Activity.
- [ ] Email is optional per user; Activity remains enabled by default and email is opt-in.
- [ ] A `Do Not Notify Watchers` choice on a create or edit suppresses both Activity and email for that change. It does not suppress revision history or audit records.
- [ ] The local suppression meaning is selected contract behavior. The captured source control's server-side behavior is unverified and is not asserted as provenance.
- [ ] A user with overlapping subscriptions receives at most one notification for one change. **Proposed necessary functional semantics:** deduplicate before Activity/email delivery.

### Transaction and history boundaries

- [ ] A failed create or edit transaction creates no watcher event or notification. **Proposed necessary functional semantics:** event creation participates in the successful change boundary.
- [ ] Historical import, backfill, and replay do not generate watcher notifications. **Proposed necessary functional semantics:** notifications apply only to new local events.

## How it works

- [Existing relation](../../deepwell/src/services/relation/page_watch.rs)
- [Account email configuration](cobalt-accounts.md)
- [Editor watcher-control gap](cobalt-form-editor.md)
- [Current replica status](../wiki/systems/cobalt-replica-status.md)

## Implementation inventory

- `deepwell/src/services/relation/page_watch.rs` — existing user-to-page `PageWatch` relation; no delivery flow is implemented.
- `deepwell/src/services/email/mailgun.rs` — existing Mailgun sender to reuse; watching does not add a second mail service.
- Site and category watch models, APIs, UI, event delivery, and notification persistence do not exist.

## Tests asserting this spec

- No watcher tests exist yet.

## Known gaps (current cycle)

- [ ] Implement page subscription management using only the existing `PageWatch` relation.
- [ ] Implement permission checks for subscription, Activity, and email delivery.
- [ ] Implement create/edit watcher events, suppression, deduplication, Activity, and optional Mailgun email delivery.
- [ ] Add behavioral tests for successful changes, rollback, privacy, suppression, overlap deduplication, imported-page future edits, and no historical-backfill delivery.
- [ ] Decide unsubscribe granularity.
- [ ] Decide email payload and author self-notification policy.

## Out of scope

- Comments and forum notifications are deferred; post-comment behavior is excluded while comments are deferred.
- Site and category watches, other channels, digests, mentions, and role-management tools.
- Subscription import, automatic enrollment of migrated users, and notification delivery from historical migration/backfill.
- Exact source-server semantics for the captured `dont_notify_watchers` control.
