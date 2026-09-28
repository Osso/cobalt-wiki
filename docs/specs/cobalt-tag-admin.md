# Cobalt site tag administration

Site admins manage tags across currently live pages from a confirmed site-wide preview. Deepwell owns authorization, revision edits, durable operation records, and reversal; the site UI consumes these RPCs.

## What it must do

- [ ] Only a current site admin/root with an unrestricted session can preview, apply, list operations, or undo. Actor and site come exclusively from authenticated request context; input `ip_address` is for audit attribution.
- [ ] `site_tag_preview {source_tag,target_tag:string|null}` returns normalized names, `preview_token`, `count`, and sorted `{page_id,slug,title}` entries for every live page whose latest revision has the source tag. `null` target means deletion; identical, blank, or whitespace-containing tags are invalid.
- [ ] `site_tag_apply {source_tag,target_tag,preview_token,ip_address}` succeeds only if its current affected pages and revisions exactly match preview. It edits every affected page in one transaction through the ordinary page revision path, preserves drafts/content/history/unrelated tags, deduplicates merges, and returns `{operation_id,count}`. Empty sets, locked or noneditable pages, and stale previews cannot produce partial changes.
- [ ] `site_tag_history {}` returns newest-first `{operation_id,source_tag,target_tag,actor_user_id,created_at,count,reversed,pages}`. `pages` contains `{page_id,slug,title}`. Reversal records retain each page's exact prior/post-operation tags and result revision.
- [ ] `site_tag_undo {operation_id,ip_address}` restores exact original tag arrays using fresh revisions, without changing later edits. If **any** page changed revision, was deleted, is locked, or is no longer editable, all-or-nothing undo fails; history stays unreversed. No separate undo preview is needed: history lists affected pages.
- [ ] Error codes: `PermissionDenied` 3106, `BadRequest` 4000 (invalid names/unknown or reversed operation), `TagPreviewStale` 4304, `TagUndoConflict` 4305, `TagPageNotEditable` 4306.

## How it works

- Existing [page revisions](cobalt-page-metadata.md) and [site member authorization](cobalt-member-admin.md) define revision and actor behavior.

## Implementation inventory

- `deepwell/src/services/site_tag_admin.rs`, `deepwell/src/endpoints/site_tag_admin.rs`, `deepwell/src/api.rs` — site-scoped operations and RPC registration.
- `deepwell/src/services/page/service.rs` — ordinary page changes serialize with bulk operations before reading revisions.
- `deepwell/migrations/20260928000000_site_tag_operation.sql` — operation/page reversal records.
- `deepwell/src/error/error_type.rs` — dedicated conflict codes.

## Tests asserting this spec

- `deepwell/tests/site_tag_admin.rs` — real-DB authorization, merge, deletion, preview conflict, reversal, and later-edit conflict.

## Known gaps (current cycle)

- [ ] Prove targeted DB tests and concurrency with independent transactions; main owns UI integration and final verification.

## Out of scope

- Bulk add, automatic tagging rules, and deploying or running site-wide tag mutations against real content.
