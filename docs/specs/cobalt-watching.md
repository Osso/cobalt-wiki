# Cobalt watching

Cobalt implements local site, category, and page watching. This specification defines the implemented local contract; [replica status](../wiki/systems/cobalt-replica-status.md) records its proof and limits. Its source basis is Wikidot's [watching FAQ](https://www.wikidot.com/faq:watching), plus the historical [staff explanation](https://community.wikidot.com/forum/t-456532/what-s-going-on-with-watching) of Activity with optional email. Comments remain deferred.

## What it must do

### Subscriptions

- [x] A signed-in user may subscribe to and unsubscribe from a site, category, or page they can read.
- [x] Subscribing, viewing Activity, and receiving a notification each require current `Page/View` visibility for the affected page, using its page/category context. Site subscriptions require an existing site and unscoped `Page/View`; they do not grant page access. `Site/View` is not required. This privacy rule does not define global role policy.
- [x] New local subscriptions only: imported or migrated users are not auto-enrolled, and migration tooling does not import subscriptions.
- [x] No automatic watches exist by default. A user may opt in to automatically watch pages after editing them.
- [x] The event scope is page creation and page edits. Later edits to imported pages are ordinary future events and may notify locally subscribed users.

### Activity and email

- [x] Page changes appear in each eligible subscriber's Activity.
- [x] Email is optional per user; Activity remains enabled by default and email is opt-in.
- [x] An email contains the page title, event type, actor, time, change link, and, for an edit, an added-and-removed rendered-text diff. It does not include raw wikitext or unapproved full source content.
- [x] An edit email's diff is limited to 1,000 characters total; the change link provides the complete change.
- [x] A recipient must have current `Page/View` visibility and readable stored content for both revisions of an edit before Activity or email is delivered; historical ACL snapshots are not required. If visibility cannot be established, delivery fails closed. A creation checks only its created revision.
- [x] The acting user receives neither Activity nor email for their own change.
- [x] A user with overlapping site, category, and page subscriptions receives at most one notification for one change.
- [x] `Do Not Notify Watchers` on a create or edit suppresses both Activity and email for that change. It does not suppress revision history or audit records.
- [x] The local suppression meaning is selected contract behavior. Captured-source server behavior remains unverified and is not asserted as provenance.
- [x] A one-click unsubscribe disables watcher email for that user without changing Activity or subscriptions.

### Transaction and history boundaries

- [x] A failed create or edit transaction creates no watcher event or notification. Only committed changes notify watchers.
- [x] Historical import, backfill, and replay do not generate watcher notifications. **Proposed necessary functional semantics:** notifications apply only to new local events.

## How it works

- [Existing page-watch relation](../../deepwell/src/services/relation/page_watch.rs)
- [Account email configuration](cobalt-accounts.md)
- [Editor watcher controls](cobalt-form-editor.md)
- [Current replica status](../wiki/systems/cobalt-replica-status.md)

## Implementation inventory

- `deepwell/src/services/relation/page_watch.rs` — retained user-to-page `PageWatch` relation.
- `deepwell/src/services/watching/` — local events, delivery worker, Activity, visibility, subscriptions, preferences, bounded rendered-text diff, and runtime email-attempt recording.
- `f1d285cd3` — current backend watcher access uses normal `Page/View` policy; no invented `Site/View` requirement or grant change. Visibility is current-revision access, not historical ACL or include snapshots.
- `514af863a` — delivery and visibility refactor.
- `9b04febf3` and `a39fdd970` — frontend watch controls, Activity/detail/preferences/unsubscribe surfaces, and editor suppression across all four editor modes.
- Runtime email-attempt state is persisted before external send. Unknown outcomes are never blindly replayed: this is at-most-once attempt recording, not an exactly-once SMTP guarantee.
- SQLx owner deployment validates the local URL, reuses its existing ledger and private diagnostics, and applies the pending password-token and watch schema migrations empty, without auto-enrollment.

## Tests asserting this spec

- `/tmp/cobalt-watching-refactor-targeted.log` — 16 targeted backend tests pass: delivery 7, events 1, subscriptions 3, visibility 5. They cover committed delivery, self/import/suppression/rollback boundaries, current-revision visibility and fail-closed rendering, overlap deduplication, unsubscribe, bounded diff, opt-out, Mailgun failure non-replay, and later-delivery continuation after one recipient's rendering failure.
- `/tmp/cobalt-watching-page-policy-green.log` — 10 policy tests pass across subscription and visibility suites. It overlaps the targeted suites and is not additional distinct coverage.
- `/tmp/cobalt-watching-diff-green.log` — 6 pure bounded-diff tests pass.
- Fake Mailgun proof validates mail payloads and opt-out; recorded 500 failures are not replayed. No real email is attempted.
- `/tmp/cobalt-watching-ui.log` — local browser proof 1/1 covers watch/unwatch at all three scopes, default-off preferences, auto-watch toggle and restoration, email off, a created synthetic page (`3000006133`) with two revisions, Activity count 2 and detail before/after rendering, then cleanup to zero subscriptions.
- `/tmp/cobalt-watching-editor-green.log` — local browser proof 1/1 covers four editor modes; default and opted-out controls serialize while eight aborted saves make no writes.
- Protected local-final artifacts record unchanged original pages `6116`, native content `10092`, history `45369`, drafts `0`, and grants `32`; fixture page `3000006133` and its two revisions are excluded. Native-content hashing excludes five renderer-cache/`updated_at` fields and is not a full-row equality claim. Final cleanup records zero subscriptions, email-enabled users, auto-watch users, and real email attempts; fixture account `20000005` remains without email.

## Final integration evidence

- [x] At `63b4c5501`, the watcher implementation passes its final scoped gate. Backend `cargo fmt --check`, `cargo check`, and Rust readability pass; retained stub tests pass 23/23. Fresh changed-test ESLint/Prettier and deploy-path Ruff lint/format pass.
- [x] Backend proof is 16 targeted tests, plus 10 overlapping policy tests and 6 bounded-diff tests; the latter two counts are not additive. Local UI and editor proof each pass 1/1; editor coverage includes all four modes and eight aborted saves with no writes.
- [x] Protected local artifacts retain original pages, native content, history, drafts, and grants. Post-deploy readback records two Activity detail checks. No real email was sent: two notifications were captured with zero email attempts.
- [ ] Whole-project Svelte typecheck is not ready: five unchanged errors in `framerail/tests/local/imported-history.mjs` (lines 87, 103, 110, 158, and 164) make `svelte-check` exit 1. Watcher-scope errors are zero. Fix those errors and rerun the whole-project check before a branch-ready claim.

## Out of scope

- Comments and forum notifications are deferred.
- Other channels, digests, mentions, and role-management tools.
- Subscription import, automatic enrollment of migrated users, and notification delivery from historical migration/backfill.
- Exact source-server semantics for the captured `dont_notify_watchers` control.
