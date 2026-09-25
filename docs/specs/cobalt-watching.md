# Cobalt watching

Cobalt will support Wikidot-style watching for locally created site, category, and page subscriptions. This specification defines the required behavior; the current implementation state is described in the [replica status](../wiki/systems/cobalt-replica-status.md). Its source basis is Wikidot's [watching FAQ](https://www.wikidot.com/faq:watching), which covers edit/comment watches, preferences, unsubscribing, and unique email links, plus the historical [staff explanation](https://community.wikidot.com/forum/t-456532/what-s-going-on-with-watching) of Activity with optional email. Comments remain deferred here.

## What it must do

### Subscriptions

- [ ] A signed-in user may subscribe to and unsubscribe from a site, category, or page they can read.
- [ ] Subscribing, viewing Activity, and receiving a notification each require current read visibility for the affected page. This privacy requirement must not assume or define global role policy.
- [ ] New local subscriptions only: imported or migrated users are not auto-enrolled, and migration tooling does not import subscriptions.
- [ ] No automatic watches exist by default. A user may opt in to automatically watch pages after editing them.
- [ ] The supported initial event scope is page creation and page edits. Later edits to imported pages are ordinary future events and may notify locally subscribed users.

### Activity and email

- [ ] Page changes appear in each eligible subscriber's Activity.
- [ ] Email is optional per user; Activity remains enabled by default and email is opt-in.
- [ ] An email contains the page title, event type, actor, time, change link, and, for an edit, an added-and-removed rendered-text diff. It does not include raw wikitext or unapproved full source content.
- [ ] An edit email's diff is limited to 1,000 characters total; the change link provides the complete change.
- [ ] A recipient must have read visibility for both revisions of an edit before Activity or email is delivered. If that visibility cannot be established, delivery fails closed. A creation checks the created revision only; it has no synthetic prior revision.
- [ ] The acting user receives neither Activity nor email for their own change.
- [ ] A user with overlapping site, category, and page subscriptions receives at most one notification for one change.
- [ ] A `Do Not Notify Watchers` choice on a create or edit suppresses both Activity and email for that change. It does not suppress revision history or audit records.
- [ ] The local suppression meaning is selected contract behavior. The captured source control's server-side behavior is unverified and is not asserted as provenance.
- [ ] A one-click unsubscribe disables all watcher email for that user without changing their Activity or subscriptions.

### Transaction and history boundaries

- [ ] A failed create or edit transaction creates no watcher event or notification. Only committed changes notify watchers.
- [ ] Historical import, backfill, and replay do not generate watcher notifications. **Proposed necessary functional semantics:** notifications apply only to new local events.

## How it works

- [Existing relation](../../deepwell/src/services/relation/page_watch.rs)
- [Account email configuration](cobalt-accounts.md)
- [Editor watcher-control gap](cobalt-form-editor.md)
- [Current replica status](../wiki/systems/cobalt-replica-status.md)

## Implementation inventory

- `deepwell/src/services/relation/page_watch.rs` — existing user-to-page `PageWatch` relation; no delivery flow is implemented.
- `deepwell/src/services/email/mailgun.rs` — existing Mailgun sender to reuse; watching does not add a second mail service.
- `deepwell/src/services/watching/diff.rs` — pure, bounded rendered-text change summary; pinned `similar` 2.7.0 supplies word-level matching instead of custom quadratic matching.
- Site and category watch models, APIs, UI, event delivery, notification persistence, and one-click unsubscribe do not exist.

## Tests asserting this spec

- `deepwell/src/services/watching/diff.rs` — pure diff tests cover separated edits, creation, empty/no change, Unicode, and the combined 1,000-character limit. Delivery tests do not exist yet.

## Known gaps (current cycle)

- [ ] Implement site, category, and page subscription management.
- [ ] Implement permission checks for subscription, Activity, and delivery across revision visibility.
- [ ] Implement committed create/edit watcher events, suppression, overlapping-watch deduplication, Activity, optional Mailgun email delivery, and one-click email unsubscribe.
- [ ] Add behavioral tests for successful changes, rollback, privacy across revisions, suppression, self-notification exclusion, overlap deduplication, email payload limits, unsubscribe behavior, imported-page future edits, and no historical-backfill delivery.

## Out of scope

- Comments and forum notifications are deferred; post-comment behavior is excluded while comments are deferred.
- Other channels, digests, mentions, and role-management tools.
- Subscription import, automatic enrollment of migrated users, and notification delivery from historical migration/backfill.
- Exact source-server semantics for the captured `dont_notify_watchers` control.
