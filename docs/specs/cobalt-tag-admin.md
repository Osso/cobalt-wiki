# Cobalt site tag administration

Site admins manage tags across currently live pages from a confirmed site-wide preview. Deepwell owns authorization, revision edits, durable operation records, and reversal; the site UI consumes these RPCs.

## What it must do

- [x] Only a current site admin/root with an unrestricted session can preview, apply, list operations, or undo. Actor and site come exclusively from authenticated request context; input `ip_address` is for audit attribution.
- [x] `site_tag_preview {source_tag,target_tag:string|null}` returns normalized names, `preview_token`, `count`, and sorted `{page_id,slug,title}` entries for every live page whose latest revision has the source tag. `null` target means deletion; identical, blank, or whitespace-containing tags are invalid.
- [ ] `site_tag_apply {source_tag,target_tag,preview_token,ip_address}` succeeds only if its current affected pages and revisions exactly match preview. It edits every affected page in one transaction through the ordinary page revision path, preserves drafts/content/history/unrelated tags, deduplicates merges, and returns `{operation_id,count}`. Empty sets, locked or noneditable pages, and stale previews cannot produce partial changes.
- [x] `site_tag_history {}` returns newest-first `{operation_id,source_tag,target_tag,actor_user_id,created_at,count,reversed,pages}`. `pages` contains `{page_id,slug,title}`. Reversal records retain each page's exact prior/post-operation tags and result revision.
- [x] `site_tag_undo {operation_id,ip_address}` restores exact original tag arrays using fresh revisions, without changing later edits. If **any** page changed revision, was deleted, is locked, or is no longer editable, all-or-nothing undo fails; history stays unreversed. No separate undo preview is needed: history lists affected pages.
- [x] Error codes: `PermissionDenied` 3106, `BadRequest` 4000 (invalid names/unknown or reversed operation), `TagPreviewStale` 4304, `TagUndoConflict` 4305, `TagPageNotEditable` 4306.

## Frontend behavior

- [x] `/-/admin/tags` lets an authorized administrator choose rename/merge or delete, preview exact affected pages, then explicitly confirm the snapshot before applying it.
- [x] It lists recorded operations with affected pages and requires an explicit confirmation before undo. Signed-out and denied visitors receive no mutation form.

## How it works

- Existing [page revisions](cobalt-page-metadata.md) and [site member authorization](cobalt-member-admin.md) define revision and actor behavior.
- The [Site Manager](cobalt-site-manager.md) is the entry point; it links this local-only feature.

## Implementation inventory

- `deepwell/src/services/site_tag_admin.rs`, `deepwell/src/endpoints/site_tag_admin.rs`, `deepwell/src/api.rs` — site-scoped operations and RPC registration.
- `deepwell/src/services/page/service.rs`, `deepwell/src/services/import/service.rs` — ordinary and imported page changes serialize with bulk operations before reading revisions.
- `deepwell/migrations/20260928000000_site_tag_operation.sql` — operation/page reversal records.
- `deepwell/src/error/error_type.rs` — dedicated conflict codes.
- `framerail/src/routes/[x+2d]/admin/tags/+page.server.ts`, `framerail/src/routes/[x+2d]/admin/tags/+page.svelte` — form actions, confirmation UI, and operation history.
- `framerail/src/lib/server/deepwell/tag-admin.ts`, `framerail/src/lib/server/load/tag-admin.ts` — trusted session/site context and RPC boundary.

## Tests asserting this spec

- `deepwell/tests/site_tag_admin.rs` — isolated real-DB authorization (including member and restricted-session denial), merge/deletion, preview and later-edit conflicts, draft preservation, locked apply/undo atomicity, and serialization with ordinary edits/imports. At `114bc6918`, the targeted command passed 7/7 with no warnings; agent6's later test-only commit `3762ffc49` extends source coverage without a redundant runtime rerun.
- `framerail/tests/tag-admin.test.ts` — 7/7 action/rendering tests at `5da81ecf1`: trusted context, input validation, preview/confirmation, denial/staleness, undo, and escaped preview rendering.
- `framerail/tests/local/tag-admin.mjs` — browser fixture flow at `328d1f1f6`: 1/1 rename/merge, delete, undo, cancel, anonymous denial, content preservation, and fixture cleanup.

## Local rollout proof

Local Deepwell's running and built artifact SHA-256 values match (`e810e20b…bdaad82a8dec`) in `/tmp/cobalt-tag-local-runtime.json`. This is local deployment evidence only; it neither identifies a production artifact nor authorizes site-wide mutations.

## Known gaps (current cycle)

- [ ] The overall final gate remains pending agent10's cargo checks. This spec does not mark the feature complete.
- [ ] No production deployment or mutation proof exists.

## Out of scope

- Bulk add, automatic tagging rules, and deploying or running site-wide tag mutations against real content.
