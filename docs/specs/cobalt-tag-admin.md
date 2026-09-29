# Cobalt site tag administration

Site admins manage tags across currently live pages from a confirmed site-wide preview. Deepwell owns authorization, revision edits, durable operation records, and reversal; the site UI consumes these RPCs.

## What it must do

- [x] Only a current site admin/root with an unrestricted session can preview, apply, list operations, or undo. Actor and site come exclusively from authenticated request context; input `ip_address` is for audit attribution.
- [x] `site_tag_preview {source_tag,target_tag:string|null}` returns normalized names, `preview_token`, `count`, and sorted `{page_id,slug,title}` entries for every live page whose latest revision has the source tag. `null` target means deletion; identical, blank, or whitespace-containing tags are invalid.
- [x] `site_tag_apply {source_tag,target_tag,preview_token,ip_address}` succeeds only if its current affected pages and revisions exactly match preview. It edits every affected page in one transaction through the ordinary page revision path, preserves drafts/content/history/unrelated tags, deduplicates merges, and returns `{operation_id,count}`. Empty sets, locked or noneditable pages, and stale previews cannot produce partial changes.
- [x] `site_tag_history {}` returns newest-first `{operation_id,kind,source_tag,target_tag,filters,actor_user_id,created_at,count,reversed,pages}`. `kind` is `rename`, `delete`, or `add`; `source_tag` is null only for add, whose added tag is `target_tag`. `filters` is `{existing_tag,category,title_contains}` for add and null otherwise. Existing records are migrated to inferred rename/delete kinds. `pages` contains `{page_id,slug,title}`. Reversal records retain each page's exact prior/post-operation tags and result revision.
- [x] `site_tag_add_preview {tag:string,existing_tag:string|null,category:string|null,title_contains:string|null}` returns the same four normalized fields, `preview_token`, `count`, and sorted `{page_id,slug,title}` for live pages missing `tag` that match **all** specified filters. `site_tag_add_apply` accepts those selection fields plus `preview_token,ip_address` and returns `{operation_id,count}` after explicit confirmation. At least one nonempty filter is required. Existing tag and category are exact matches (`_default` is a valid category); title matching is case-insensitive literal substring, never source/rendered content. No OR, automatic rules, or duplicate tag additions.
- [x] Add previews bind tag, filters, exact selected pages, revisions, and tags. Apply recomputes under the site advisory lock; stale sets/revisions, empty sets, locks, or edit denial produce no partial mutations. Ordinary page revisions preserve drafts/content/history/unrelated tags; add history records exact normalized filters, and existing conservative all-or-none undo restores prior tag arrays.
- [x] `site_tag_undo {operation_id,ip_address}` restores exact original tag arrays using fresh revisions, without changing later edits. If **any** page changed revision, was deleted, is locked, or is no longer editable, all-or-nothing undo fails; history stays unreversed. No separate undo preview is needed: history lists affected pages.
- [x] Error codes: `PermissionDenied` 3106, `BadRequest` 4000 (invalid names/unknown or reversed operation), `TagPreviewStale` 4304, `TagUndoConflict` 4305, `TagPageNotEditable` 4306.

## Frontend behavior

- [x] `/-/admin/tags` lets an authorized administrator choose rename/merge or delete, preview exact affected pages, then explicitly confirm the snapshot before applying it.
- [x] It lists recorded operations with affected pages and requires an explicit confirmation before undo. Signed-out and denied visitors receive no mutation form.
- [x] Bulk add requires at least one filter; the preview shows the normalized tag and AND criteria, exact affected pages and count. Confirmation uses the preview token, cancel does not mutate, and history shows add filters and undo.

## How it works

- Existing [page revisions](cobalt-page-metadata.md) and [site member authorization](cobalt-member-admin.md) define revision and actor behavior.
- The [Site Manager](cobalt-site-manager.md) is the entry point.

## Implementation inventory

- `deepwell/src/services/site_tag_admin.rs`, `deepwell/src/endpoints/site_tag_admin.rs`, `deepwell/src/api.rs` — site-scoped operations and RPC registration.
- `deepwell/src/services/page/service.rs`, `deepwell/src/services/import/service.rs` — ordinary and imported page changes serialize with bulk operations before reading revisions.
- `deepwell/migrations/20260928000000_site_tag_operation.sql`, `deepwell/migrations/20260929000000_site_tag_add.sql` — operation/page reversal records and typed add history.
- `deepwell/src/error/error_type.rs` — dedicated conflict codes.
- `framerail/src/routes/[x+2d]/admin/tags/+page.server.ts`, `framerail/src/routes/[x+2d]/admin/tags/+page.svelte` — form actions, confirmation UI, and operation history.
- `framerail/src/lib/server/deepwell/tag-admin.ts`, `framerail/src/lib/server/load/tag-admin.ts` — trusted session/site context and RPC boundary.

## Tests asserting this spec

- `deepwell/tests/site_tag_admin.rs` — isolated real-DB authorization (including member and restricted-session denial), merge/deletion/add, AND filters with `_default` and literal title matching, preview and later-edit conflicts, draft preservation, locked apply/undo atomicity, and serialization with ordinary edits/imports. Current targeted suite: 13/13 at backend `f74ad6a6d`, test revision `1bebf777d` (`/tmp/cobalt-tag-add-green6.log`).
- `framerail/tests/tag-admin.test.ts` — 11/11 action/rendering tests at frontend `1dd3f7d4b`; the four bulk-add cases cover literal filter forwarding, required filter validation, confirmed snapshot forwarding, and add-history preview/undo (`/tmp/cobalt-tag-add-ui-green.log`).
- `framerail/tests/local/tag-admin.mjs` — local browser fixtures: 2/2 at backend `f74ad6a6d` / frontend `1dd3f7d4b`; retained rename/delete/undo and bulk add verify AND selection, literal `%`/`_` title matching, source/category exclusions, already-tagged exclusion, history filters, undo, content preservation, and cleanup (`/tmp/cobalt-tag-add-browser-green.log`).

## Local rollout proof

Local `deploy.sh` installed backend `f74ad6a6d`; running and built artifact SHA-256 values match (`04039d16…4dc061`) in `/tmp/cobalt-tag-add-local-runtime.json`. This is local deployment evidence only; production was unchanged. Browser tests used new, randomly tagged fixture pages and removed them afterward.

## Verification

- Final Rust gate: `cargo fmt --check` and `cargo check` exit 0 with no warnings; current backend production source and migration match `f74ad6a6d`. Receipt: `/tmp/cobalt-tag-add-final-gate/cargo-final-gate-receipt.txt`.
- Frontend: Svelte diagnostics report 0 errors and 0 warnings; scoped ESLint, Prettier, and prior scoped Stylelint pass. The check output retains existing missing-route-prefix notices, so this is not a claim of zero inherited build output. Current action/rendering suite: 11/11; corrected touched bulk-add cases: 4/4 (`/tmp/cobalt-tag-add-final-gate/frontend-final-gate-receipt-ci.txt`, `/tmp/cobalt-tag-add-ui-correction-green.log`).
- Readability metrics found no cognitive-complexity or cyclomatic violation. `add_apply()` is approximately 43 body lines, above the 30-line advisory threshold; extraction was rejected because its visible straight-line preflight-before-write sequence is the required all-or-none ordering, not a complexity defect.

## Production rollout

Verified 2026-09-29 UTC: `install/dev-deploy.sh all` deployed `f77f6976e79a` (rename/merge/delete). Deepwell's installed SHA-256 matches the deploy artifact (`758f2d51…86f8675`); all 570 Framerail build files match. Services are active, post-start logs contain no startup/migration/tag errors, and anonymous preview/history are denied with 3106. Root, roster and the tag-admin sign-in screen return 200 with no browser errors. No production tag changes or authenticated admin operations were exercised. Receipt: `/tmp/cobalt-tag-production-verification.json`. No Sentry integration is configured; existing unrelated build warnings remain recorded there.

## Known gaps (current cycle)

Bulk-add is locally complete and verified, but intentionally not deployed to production. The production revision above remains rename/merge/delete only. No production add operation, authenticated admin workflow, or Sentry integration evidence exists; no Sentry-clean claim is made.

## Out of scope

- Automatic tagging rules and real-content site-wide mutations. Bulk-add production rollout requires separate authorization.
