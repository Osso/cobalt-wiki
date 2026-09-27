# Cobalt page actions

Bottom-page controls follow supplied Tags/History screenshots and retained Wikidot behavior. This contract covers the Wikidot layout; [history listing](cobalt-history-listing.md) and [history import](cobalt-history-import.md) define history data and provenance.

## What it must do

### Bottom actions

- [x] Render Edit, Tags, History, Files, Backlinks, Print, and More options.
- [x] Omit Vote in the Wikidot layout; preserve voting and Tags controls in other layouts.
- [x] Provide separate imported/local History datasets, seven filters, five page sizes, numbered pagination, source, read-only preview, and same-origin comparison.
- [x] History table names flag letters as chips (N New, S Source, T Title, R Renamed, A Tags, M Metadata, F Files; `data-flag` keeps the letter), labels actions View/Wikitext, links authors with a local account to `/-/user/<slug>` (others stay plain text), labels compare radios From/To with matching row edges, and shows `D Mon YYYY, HH:MM` in UTC. Controls sit in one toolbar with filter pills. `framerail/tests/history-table.test.ts` asserts chips, labels, and time.
- [x] Open and close Source (options and passive panels), Files, Parent, Move, Delete, Lock, and Layout panels in local browser passes without unapproved writes.
- [x] Read Files, Parent, and Lock panel data without writes: `?/fileList`, `?/parentGet`, and `?/lockHistory` return `200`.
- [x] Delete uses a native dialog rather than `window.confirm`, with explicit close callbacks; Cancel, Escape, and accepted confirmation are covered while the DELETE request is intercepted, and Move remains immediate.
- [x] Show viewer-filtered Backlinks links and inclusions, including an empty inclusions result.
- [x] Require explicit Delete confirmation before its POST path; both acceptance and dismissal are exercised with every mutation intercepted, so no page is deleted.
- [x] Layout changes require the authenticated actor's `Page/Edit` permission on the actual target page and site. A denied editor's browser action returns `403` without changing the disposable target's layout, source, or revision.
- [x] The disposable page-action lifecycle moves, changes layout, deletes, and restores only its manifest page, then restores its baseline; no external reads are blocked. This is bounded lifecycle proof, not proof of every action or full Move dependency repair.
- [x] Native Restore UI: the existing control on a deleted-page `404` persists recovery. The pre-fix selected radio posted `pageId: 0` and returned `500`/`Page does not exist`; `eb1384e4a` binds the selected radio to the submitted page ID. The saved lifecycle GREEN is 1/1: it asserts the manifest page ID in the Restore POST, HTTP `200` success, disappearance only after that response, and persisted source/layout/slug/revision readback. Reconstructing Wikidot's alternate Restore radio is not this contract.
- [x] Use one `Page/Edit` permission for page delete and restore. `c5262aae2` has isolated 6/6 authorization proof; its initial RED was environment-blocked, not a behavioral RED.
- [x] Require `Page/Edit` for every file mutation: upload, rename/edit, byte replacement, revision-history read, rollback, move, delete, and restore; `fileMove` requires it on both source and destination pages. No distinct File grant exists or is required. `deepwell/tests/file_permission.rs` passes 3/3 after `e974e6f84` initializes the rollback creation revision at `0` (`/tmp/claude/cobalt-move-file-targeted.log`).
- [x] For each selected Move dependency, require `Page/Edit` on both source and destination categories and on every selected dependency. Skip active native locks; preserve selected unauthorized, locked, unselected, and otherwise ineligible dependencies; report remaining link and include leftovers separately. Duplicate selected sources produce one normal actor-attributed automatic revision. The Move writes its revision directly and does not consume drafts. `deepwell/tests/page_move_dependencies.rs` passes 4/4 in the same targeted run.

### Block

- [x] An active Wikidot lock denies protected mutations unless the actual site role is moderator, administrator, or root. Native `PermissionOnly` and `AuthorOr` lock behavior remain unchanged; no nonexistent global supermoderator representation is claimed.
- [x] Offer one Block checkbox in the Wikidot action pane; its action and checked state reflect the current page lock, and browser proof persists both set and clear.
- [x] Block guards page edit, rollback, Move, Delete, Restore, Layout, every file mutation, and Parent, including the formerly bypassing single lock set/remove endpoints. `Page/Edit` remains the normal target-page authorization; Block is additional policy.

### Tags

- [x] Render Page Tags, explanatory links, labeled form-table input, space-separated hint, and close/clear/save controls.
- [x] Display Wikidot tags sorted; retain set-based additions/removals rather than promise backend array order.
- [x] Clear empties only the input; close discards unsaved input. Neither sends tag changes.
- [x] Save submits the exact additions/removals. The exercised authorized fixture saves, reads back, and restores its original tag set without changing page source.
- [ ] Independently exercise denied tag-save authorization; successful authorized fixture testing does not prove denial behavior.

## How it works

- [Page backlinks](cobalt-page-backlinks.md) defines the permission-filtered incoming-link read API.
- [Page files](cobalt-page-files.md), [page parents](cobalt-page-parent.md), and [page print view](cobalt-page-print.md) define their respective pane and route contracts. File guards have targeted 3/3 proof, Delete/Restore guards isolated 6/6, and Parent actor/site/body-child authorization isolated 9/9 plus current browser coverage. Block is additional Wikidot-layout policy, not a replacement for `Page/Edit`.

## Implementation inventory

- `framerail/src/routes/[slug]/[...extra]/+page.svelte` — bottom action registration and panels.
- `framerail/src/routes/[slug]/[...extra]/FilePane.svelte` — Files list and information view.
- `framerail/src/routes/[slug]/[...extra]/ParentPane.svelte` — parent editor.
- `framerail/src/routes/[slug]/[...extra]/BlockPane.svelte` — Wikidot Block checkbox pane.

## Tests asserting this spec

- `deepwell/tests/page_layout_permission.rs` — isolated Layout authorization, target binding, default-layout reset, and Parent authorization; Layout was 4/4 before test-only `unused_mut` cleanup, while Parent is isolated 9/9 at `ba73f942f`.
- `deepwell/tests/page_delete_restore_permission.rs` — isolated Delete/Restore authorization 6/6 at `c5262aae2`; its initial RED was blocked by the environment, not behavioral evidence.
- `deepwell/tests/file_permission.rs` — file endpoint `Page/Edit` coverage; targeted 3/3 GREEN after `e974e6f84` initializes rollback creation revision `0`.
- `deepwell/tests/page_move_dependencies.rs` — selected Move dependency authorization, preservation, native-lock skip, duplicate-source revision, direct-revision/draft, and split-leftover coverage; targeted 4/4 GREEN.
- Mutation authorization tests — 27/27 pass. Block policy and native-lock preservation have separate coverage in the 12/12 `member_admin` target.
- `framerail/tests/page-bottom-tags.test.ts` — SSR bottom-action and Tags behavior.
- `framerail/tests/local/page-bottom-actions.mjs` — Tags save/readback/restore with a fixture-only write guard.
- `framerail/tests/local/history-actions.mjs` — local History browser coverage.
- `framerail/tests/local/remaining-actions.mjs` — local panel, Files, Parent, Backlinks, and Print browser coverage.
- `framerail/tests/local/delete-confirmation.mjs` — native Delete dialog Cancel/Escape/acceptance with mutation interception; Move remains immediate.
- `framerail/tests/local/page-action-mutations.mjs` — disposable-page mutation lifecycle regression; scoped Native Restore and styled Move/Layout/Delete/Native Restore roundtrips each pass 1/1. Full Move dependency repair remains outside that scope.

## Evidence

- `/tmp/claude/cobalt-remaining-actions-parent-ready.log` — controlled delayed-GET local coverage; 1/1 pass for Source options/passive panels, parent lookup, two-parent save/readback/restore, and all remaining Files, Backlinks, and Print panels.
- `/tmp/claude/cobalt-delete-dialog-browser.log` — native Delete dialog Cancel/Escape/acceptance with the accepted DELETE intercepted; Move remains immediate; 1/1 pass at `931ca6f78`.
- `/tmp/claude/cobalt-page-action-mutations-browser.log` — earlier main disposable-page run after `2e815055d`: first Move succeeded, then the four blocked foreign GET assertion failed; recovery returned without error. Superseded for the styled action scope by the later GREEN below; it remains evidence that full Move dependency repair was not proved.
- `/tmp/claude/cobalt-page-action-styled.log` — actual styled lifecycle GREEN 1/1 at `ca141f0b3`: the stylesheet-GET guard permits no external reads, and Move, Layout, Delete, and Native Restore complete a persisted disposable-fixture roundtrip.
- `/tmp/claude/cobalt-{delete-dialog,backlinks,print}-final.png` — local captures of the exercised Delete, Backlinks, and Print states; captures record presentation only, not mutation proof.
- `/tmp/claude/cobalt-parent-loading-red.json` — reproduced pre-baseline editable-input race (`typedRetained: false`, `afterEmpty: true`); `71ce6afa2` disables editing until the baseline loads.
- `/tmp/claude/cobalt-action-preservation-gate.json` — read-only artifact comparison PASS: baseline unchanged for 6,116 pages, 45,369 history rows, zero drafts, 32 grants, and 10,091 native-content rows. SQL excludes three page identities (including sacrificial pages `3000006134` and `3000006135`) and five native cache fields. Files are outside this query; 43 original full-row fingerprints remain unreconstructable, not an additional SQL row exclusion.
- `/tmp/claude/cobalt-remaining-runtime.json` — running Deepwell SHA equals built `fc3e48773`.
- `/tmp/claude/cobalt-layout-authorization-deploy.log` — local `./deploy.sh` for `3fa1838a3` exited `0` after a compact 3m27s build; the retained log is empty.
- `/tmp/claude/cobalt-layout-denial-postfix.json` — post-deploy denied editor action against disposable page `3000006135`: Svelte failure `403`, with unchanged layout, source, and revision; fixture restored.
- **Production application rollout, 2026-09-26:** `install/dev-deploy.sh all` exited `0` and deployed application revision `16ffc05e2b8049b90e0de9324741522d518857e7` to Deepwell and Framerail under `/var/lib/cobalt-wiki/app` (`/tmp/claude/cobalt-actions-production-deploy.log`). This was not a root `deploy.sh` run, host NixOS switch, or Sakuin flake-pin update; WWS and host configuration are unchanged. Read-only public proof returned `200` for `/`, `/roster`, and `/printer--friendly/roster`; Move dependencies were visible, anonymous Block management was disabled, and the Edit permission read succeeded, with zero blocked writes and page errors (`/tmp/claude/cobalt-actions-public-browser.log`). Independent final gate `/tmp/claude/cobalt-actions-production-final-gate.json` is `PASS`, superseding the original keyword-only journal `FAIL` gate. It confirms installed hashes, active services, public routes, anonymous `can_edit=false`, watching-read denial, and co-tenant readiness. This remains read-only production proof: no production test-data mutations occurred, so action lifecycles and authenticated mutation behavior retain their prior local proof.
- `/tmp/claude/cobalt-layout-auth-gate.json` — `cargo fmt --check`, offline check, and isolated Layout authorization 4/4 pass; the pre-cleanup test warned on `unused_mut`, fixed test-only by `6f3d99463`, superseded by the passing final backend and frontend gates below.
- `/tmp/claude/cobalt-page-action-restore-post-evidence.log` — RED: selected native Restore radio for fixture `3000006134` posted `pageId: 0`; backend returned `500`/`Page does not exist`.
- `/tmp/claude/cobalt-page-action-restore-binding.log` — actual local lifecycle GREEN 1/1 after `eb1384e4a`: Restore POST carries the manifest page ID, returns HTTP `200` success, then the form disappears and persisted readback verifies recovery.
- `/tmp/claude/cobalt-native-restore-gate.json` — independent scoped audit at `a81fed8e0dc4c6bd1f4dacfd97f6de28d206f985`: the restore binding and lifecycle-test sources are unchanged from their commits; scoped Prettier, ESLint, Stylelint, and diff checks pass. It deliberately excludes a browser rerun, runtime/deploy work, whole-project typecheck, other actions, Move/Block, and denials.
- `/tmp/claude/cobalt-move-file-targeted.log` — Move dependency tests 4/4 and file authorization tests 3/3 GREEN after `e974e6f84` creates rollback revisions with creation revision `0`.
- `/tmp/claude/cobalt-move-rpc-serialized-red.log` — corrected serialized RED 2/2: required Move fields were absent. The prior initial RED used an invalid form and is not behavioral evidence.
- `/tmp/claude/cobalt-move-rpc-final-green.log` — historical Move RPC/UI serialization GREEN 18/18 after `facf4cf28`; `4367827bc` supplies authenticated request context.
- `/tmp/claude/cobalt-final-backend-followup.json` — independent backend follow-up PASS at `09cf405b7`: isolated environment, `cargo fmt --check`, offline locked `cargo check`, and `member_admin` 12/12; reused unaffected proof includes locked mutations 27/27, self-slug 1/1, and file-history authorization 3/3. Rust readability refactors `be35c3ec3`/`cff164092` do not change browser transport or UI.
- `/tmp/claude/cobalt-final-actions-runtime.json` — local runtime and build SHA-256 match at `09cf405b7`; explicitly not production.
- `/tmp/claude/cobalt-current-page-action-mutations.log` — current disposable page lifecycle GREEN 1/1; manifest-only Move/Layout/Delete/Native Restore returns to baseline with no blocked external reads.
- `/tmp/claude/cobalt-move-block-first-browser.log` — current browser GREEN 1/1 for selected Move link repair/leftovers and Block set/clear.
- `/tmp/claude/cobalt-parent-postauth-browser.log` — current browser GREEN 1/1 for remaining authorized page actions, including Parent fixture restoration.
- `/tmp/claude/cobalt-page-action-denials-textplain.log` — observer browser denials GREEN 2/2: page Move/Delete/Parent/Layout/Tags/Block and file Delete/Edit/Move. Other file-denial actions have isolated proof only.
- `/tmp/claude/cobalt-final-helper-tests.log` — current helper/transport assertions GREEN 28/28.
- `/tmp/claude/cobalt-final-action-preservation.json` — refreshed SQL comparison preserves equal hashes/counts for 6,116 pages, 45,369 imported history rows, 10,091 native-content rows, 32 grants, and zero drafts. SQL excludes three page identities and documented renderer fields; Files are outside the query and browser lifecycles separately preserve original fixture files. The 43 unreconstructable original full-row fingerprints remain outside full-row preservation claims.

## Known gaps (current cycle)

- Move retains the source include regex: colon-qualified and parameterized includes remain reported rather than rewritten. Triple links support category colons. Browser proof covers selected links and leftovers; supported includes have isolated behavioral proof.
- [ ] Browser observer denial covers page Move/Delete/Parent/Layout/Tags/Block and file Delete/Edit/Move only. Remaining file-denial actions are isolated proof, not browser proof. Lifecycle runs and SQL preservation remain bounded as stated above.
- [ ] Clear-then-Save is not separately proved beyond Parent’s two-value save/readback/restore. Delete-dialog acceptance remains intercepted; Native Restore covers only the selected existing-control recovery scope.
- [x] Frontend gate passes with `/tmp/claude/cobalt-current-frontend-final-gate.json` plus its final formatting receipt `/tmp/claude/cobalt-final-format-gate.json`: Svelte 0 errors/0 warnings, no new readability exceedances, helper tests 28/28, retained browser tests 6/6. ESLint has zero errors and one inherited warning. These local results do not establish full-replica parity.

## Out of scope

- Native regex rewrite limitations persist.
- Wikidot's single-parent source model is not substituted for Cobalt's existing plural-parent model.
- Legacy upload-storage metadata unavailable from current file responses is not invented.
- This bounded application rollout does not authorize source-site writes, source-editor side effects, imported rollback, a host deployment, or unrelated backend/non-Wikidot changes. Independent production verification remains pending.
