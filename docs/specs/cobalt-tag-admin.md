# Cobalt site tag administration

Site admins manage tags across currently live pages from a confirmed site-wide preview. Deepwell owns authorization, revision edits, durable operation records, and reversal; the site UI consumes these RPCs.

## What it must do

- [x] Only a current site admin/root with an unrestricted session can preview, apply, list operations, or undo. Actor and site come exclusively from authenticated request context; input `ip_address` is for audit attribution.
- [x] `site_tag_preview {source_tag,target_tag:string|null}` returns normalized names, `preview_token`, `count`, and sorted `{page_id,slug,title}` entries for every live page whose latest revision has the source tag. `null` target means deletion; identical, blank, or whitespace-containing tags are invalid.
- [x] `site_tag_apply {source_tag,target_tag,preview_token,ip_address}` succeeds only if its current affected pages and revisions exactly match preview. It edits every affected page in one transaction through the ordinary page revision path, preserves drafts/content/history/unrelated tags, deduplicates merges, and returns `{operation_id,count}`. Empty sets, locked or noneditable pages, and stale previews cannot produce partial changes.
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

- `deepwell/tests/site_tag_admin.rs` — isolated real-DB authorization (including member and restricted-session denial), merge/deletion, preview and later-edit conflicts, draft preservation, locked apply/undo atomicity, and serialization with ordinary edits/imports. The targeted suite passed 8/8 at `3762ffc49`. The merge regression passed again at `c9820eb69` with an existing duplicate destination, preserving unrelated duplicates and restoring the exact original array on undo.
- `framerail/tests/tag-admin.test.ts` — 7/7 action/rendering tests at `5da81ecf1`: trusted context, input validation, preview/confirmation, denial/staleness, undo, and escaped preview rendering.
- `framerail/tests/local/tag-admin.mjs` — browser fixture flow at `a3c84f1f4`, backend `c9820eb69`: 1/1 rename/merge (including duplicate destination tags), delete, undo, cancel, anonymous denial, content preservation, and fixture cleanup. Receipt: `/tmp/cobalt-tag-ui-live-final.log`.

## Local rollout proof

Local `deploy.sh` installed backend `c9820eb69`; running and built artifact SHA-256 values match (`525d438c…42f5fbf4`) in `/tmp/cobalt-tag-local-runtime-final.json`. This is local deployment evidence only; production was unchanged. Browser tests used new, randomly tagged fixture pages and removed them afterward.

## Verification

- Independent Rust gate at `a3c84f1f4` confirms backend source unchanged from `c9820eb69`; `cargo fmt --check` and `cargo check` exit 0 with no warnings. Receipt: `/tmp/cobalt-tag-final-rust-receipts.txt`.
- Frontend typecheck: 0 errors/warnings; scoped action tests: 7/7; scoped lint, formatting, and styles pass. Final browser fixture lint/format receipt: `/tmp/cobalt-tag-final-browser-style.log`.
- Readability audit found no complexity violations. The straight-line apply/undo orchestration remains together to keep preflight-before-write ordering visible; line-count-only extraction suggestions were not adopted.

## Known gaps (current cycle)

None for local acceptance. Production rollout is outside this implementation's verified scope.

## Out of scope

- Bulk add, automatic tagging rules, and deploying or running site-wide tag mutations against real content.
