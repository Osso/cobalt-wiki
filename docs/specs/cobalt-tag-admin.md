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
- [x] While a preview, apply, or undo request runs, the submitted form shows a spinner, the operation with its page count (e.g. `Renaming a to b on 2 pages…`), and elapsed seconds; all tag-admin controls stay disabled until it finishes.

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
- `framerail/tests/tag-admin.test.ts` — 11/11 action/rendering tests for `d3a4ad897`; the four affected bulk-add cases passed again at `1dd3f7d4b`. They cover literal filter forwarding, required filter validation, confirmed snapshot forwarding, and add-history preview/undo (`/tmp/cobalt-tag-add-ui-green.log`, `/tmp/cobalt-tag-add-ui-correction-green.log`).
- `framerail/tests/local/tag-admin.mjs` — local browser fixtures: 2/2 at backend `6ee6f7dc3` / frontend `4e31dc6b7` (`/tmp/claude/tag-progress-green.log`); the rename apply is held in the browser to assert the in-flight label, disabled confirm, and ticking elapsed time (fails against the prior `Working…` page: `/tmp/claude/tag-progress-red.log`); retained rename/delete/undo and bulk add verify AND selection, literal `%`/`_` title matching, source/category exclusions, already-tagged exclusion, history filters, undo, content preservation, and cleanup (`/tmp/cobalt-tag-add-browser-green.log`).

## Local rollout proof

Local `deploy.sh` installed backend `f74ad6a6d`; running and built artifact SHA-256 values match (`04039d16…4dc061`) in `/tmp/cobalt-tag-add-local-runtime.json`. This is local deployment evidence only; production was unchanged. Browser tests used new, randomly tagged fixture pages and removed them afterward.

## Verification

- Final Rust gate: `cargo fmt --check` and `cargo check` exit 0 with no warnings; current backend production source and migration match `f74ad6a6d`. Receipt: `/tmp/cobalt-tag-add-final-gate/cargo-final-gate-receipt.txt`.
- Frontend: Svelte diagnostics report 0 errors and 0 warnings; scoped ESLint, Prettier, and prior scoped Stylelint pass. The check output retains existing missing-route-prefix notices, so this is not a claim of zero inherited build output. Current action/rendering suite: 11/11; corrected touched bulk-add cases: 4/4 (`/tmp/cobalt-tag-add-final-gate/frontend-final-gate-receipt-ci.txt`, `/tmp/cobalt-tag-add-ui-correction-green.log`).
- Readability metrics found no cognitive-complexity or cyclomatic violation. `add_apply()` is approximately 43 body lines, above the 30-line advisory threshold; extraction was rejected because its visible straight-line preflight-before-write sequence is the required all-or-none ordering, not a complexity defect.

## Production rollout

Verified 2026-09-29 UTC: `install/dev-deploy.sh all` deployed `ac963e66eb46b153032aa825def691385b322ff4` (deploy log: `Deployed all (ac963e66eb46)`). Fresh artifact comparisons after the SSH-banner timeout show zero differences: installed Deepwell matches `/syncthing/Sync/Projects/cobalt-company-wiki/deepwell/target/nix-deploy/deploy/deepwell` at SHA-256 `69b22f8e2fc249a9e62b504b6970cef3ec71f1cb14283a1f7e12c00c265974a7`, and the installed Framerail build matches its local build. Both services are active; Deepwell reports `v2026.8.20 [ac963e66]`; and startup applied `20260929000000_site_tag_add.sql` (`migrate site tag add`). The installed migration file matches SHA-256 `11099d2743ebaa1b569aaeb795f9ab9269d458507e68c05edaffbd41814d7b0a`.

Anonymous proof: a cookie-free `GET https://cobalt-company.sakuin.org/-/admin/tags` returned 200, while cookie-free JSON-RPC `site_tag_add_preview` and `site_tag_add_apply` both returned `PermissionDenied` 3106. No authenticated tag mutation or authenticated admin workflow was exercised. No Sentry integration is configured, so no Sentry-clean claim is made. Receipt: `/tmp/cobalt-tag-add-production-verification.json`; deploy log: `/tmp/cobalt-tag-add-production-deploy.log`.

Progress indicator, verified 2026-09-29 UTC: `install/dev-deploy.sh framerail` deployed `c8e9f6d74` (`/tmp/claude/tag-progress-prod-deploy.log`); `rsync -rnc` shows zero build differences and both services are active. Signed in as `cobalt-import` on `https://cobalt-company.sakuin.org/-/admin/tags`, a held delete preview of an unused random tag showed `Finding affected pages…`, a disabled submit, and ticking seconds, then `Preview: 0 pages` with no confirm button. No tag mutation was made.

## Known gaps (current cycle)

Bulk-add is deployed, but no production add operation or authenticated admin workflow was exercised. No Sentry integration evidence exists.

Throughput (measured 2026-09-29, local replica data, prod-identical optimized backend `60d7abaf6`, 1,000 one-line fixture pages): apply 56–73 s and undo 60–74 s, ≈0.06–0.07 s/page, all in one request holding the site tag lock that ordinary edits and imports wait on. Per page: nav top bar re-render 39 ms (`nav:top` includes `:snippets:suo` showto), full-site `page_listing` scan in `outdate_listings` 26 ms (6,668 listings re-read and JSON-decoded per edit), other work ≈6 ms, fixture body render 0.7 ms (real bodies not measured). Implied: 5,925 pages ≈ 6–7 min, exceeding Cloudflare's usual 100 s origin timeout (inferred, not tested) and Node fetch's 300 s default between Framerail and Deepwell. Timing instrumentation patch: `/tmp/claude/tag-apply-profiling.patch` (not committed). The unrelated wildcard-language limitation is tracked in [Site Manager gaps](cobalt-site-manager.md#known-gaps-current-cycle).

## Out of scope

- Automatic tagging rules and real-content site-wide mutations.
