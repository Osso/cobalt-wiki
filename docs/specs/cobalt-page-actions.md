# Cobalt page actions

Bottom-page controls follow supplied Tags/History screenshots and retained Wikidot behavior. This contract covers the Wikidot layout; [history listing](cobalt-history-listing.md) and [history import](cobalt-history-import.md) define history data and provenance.

## What it must do

### Bottom actions

- [x] Render Edit, Tags, History, Files, Backlinks, Print, and More options.
- [x] Omit Vote in the Wikidot layout; preserve voting and Tags controls in other layouts.
- [x] Provide separate imported/local History datasets, seven filters, five page sizes, numbered pagination, source, read-only preview, and same-origin comparison.
- [x] Open and close Source (options and passive panels), Files, Parent, Move, Delete, Lock, and Layout panels in local browser passes without unapproved writes.
- [x] Read Files, Parent, and Lock panel data without writes: `?/fileList`, `?/parentGet`, and `?/lockHistory` return `200`.
- [x] Delete uses a native dialog rather than `window.confirm`, with explicit close callbacks; Cancel, Escape, and accepted confirmation are covered while the DELETE request is intercepted, and Move remains immediate.
- [x] Show viewer-filtered Backlinks links and inclusions, including an empty inclusions result.
- [x] Require explicit Delete confirmation before its POST path; both acceptance and dismissal are exercised with every mutation intercepted, so no page is deleted.
- [x] Layout changes require the authenticated actor's Page/Edit permission on the actual target page and site. After local deployment of `3fa1838a3`, a denied editor's real browser action returns Svelte failure `403`; the disposable target's layout, source, and revision remain unchanged. The isolated authorization test passed 4/4 before its test-only `unused_mut` cleanup; its targeted recheck is pending.
- [ ] Apart from guarded Tags and Parent save/readback/restore, the bounded Layout denial, the complete disposable Files lifecycle, and the scoped Move/Layout/Delete/Native Restore roundtrip below, panel mutations and authorization remain unproven. The styled lifecycle GREEN 1/1 has no blocked external stylesheet reads and restores its disposable fixture, but it does not prove full Move dependency repair, all actions, or authorization. The separate Delete-dialog acceptance remains intercepted.
- [x] Native Restore UI: the existing control on a deleted-page `404` persists recovery. The pre-fix selected radio posted `pageId: 0` and returned `500`/`Page does not exist`; `eb1384e4a` binds the selected radio to the submitted page ID. The saved lifecycle GREEN is 1/1: it asserts the manifest page ID in the Restore POST, HTTP `200` success, disappearance only after that response, and persisted source/layout/slug/revision readback. Reconstructing Wikidot's alternate Restore radio is not this contract.
- [x] Use one `Page/Edit` permission for page delete and restore and every file mutation: upload, rename/edit, byte replacement, revision-history read, rollback, move, delete, and restore. `fileMove` requires `Page/Edit` on both source and destination pages. No distinct File grant exists or is required.

### Tags

- [x] Render Page Tags, explanatory links, labeled form-table input, space-separated hint, and close/clear/save controls.
- [x] Display Wikidot tags sorted; retain set-based additions/removals rather than promise backend array order.
- [x] Clear empties only the input; close discards unsaved input. Neither sends tag changes.
- [x] Save submits the exact additions/removals. The exercised authorized fixture saves, reads back, and restores its original tag set without changing page source.
- [ ] Independently exercise denied tag-save authorization; successful authorized fixture testing does not prove denial behavior.

## How it works

- [Page backlinks](cobalt-page-backlinks.md) defines the permission-filtered incoming-link read API.
- [Page files](cobalt-page-files.md), [page parents](cobalt-page-parent.md), and [page print view](cobalt-page-print.md) define their respective pane and route contracts. The parent update’s actual request actor/site and body-child `Page/Edit` check has isolated 9/9 proof at `ba73f942f`; it has not been deployed or exercised in a browser.

## Implementation inventory

- `framerail/src/routes/[slug]/[...extra]/+page.svelte` — bottom action registration and panels.
- `framerail/src/routes/[slug]/[...extra]/FilePane.svelte` — Files list and information view.
- `framerail/src/routes/[slug]/[...extra]/ParentPane.svelte` — parent editor.

## Tests asserting this spec

- `deepwell/tests/page_layout_permission.rs` — isolated Layout authorization, target binding, and default-layout reset; 4/4 passed before test-only `unused_mut` cleanup, with targeted recheck pending.
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
- `/tmp/claude/cobalt-remaining-preservation.json` — preserved: 6,116 original pages, 45,369 imported records, 10,091 native-content rows, 32 grants, and zero drafts. This proof predates sacrificial pages `3000006134` and `3000006135`, so its original-page count does not cover those fixtures; it excludes the earlier sacrificial Tags/Parent fixture and native cache fields.
- `/tmp/claude/cobalt-remaining-runtime.json` — running Deepwell SHA equals built `fc3e48773`.
- `/tmp/claude/cobalt-layout-authorization-deploy.log` — local `./deploy.sh` for `3fa1838a3` exited `0` after a compact 3m27s build; the retained log is empty.
- `/tmp/claude/cobalt-layout-denial-postfix.json` — post-deploy denied editor action against disposable page `3000006135`: Svelte failure `403`, with unchanged layout, source, and revision; fixture restored.
- `/tmp/claude/cobalt-layout-auth-gate.json` — `cargo fmt --check`, offline check, and isolated Layout authorization 4/4 pass; the pre-cleanup test warned on `unused_mut`, fixed test-only by `6f3d99463`, so targeted recheck remains pending. Scoped Prettier and ESLint pass; current frontend type proof is pending.
- `/tmp/claude/cobalt-page-action-restore-post-evidence.log` — RED: selected native Restore radio for fixture `3000006134` posted `pageId: 0`; backend returned `500`/`Page does not exist`.
- `/tmp/claude/cobalt-page-action-restore-binding.log` — actual local lifecycle GREEN 1/1 after `eb1384e4a`: Restore POST carries the manifest page ID, returns HTTP `200` success, then the form disappears and persisted readback verifies recovery.
- `/tmp/claude/cobalt-native-restore-gate.json` — independent scoped audit at `a81fed8e0dc4c6bd1f4dacfd97f6de28d206f985`: the restore binding and lifecycle-test sources are unchanged from their commits; scoped Prettier, ESLint, Stylelint, and diff checks pass. It deliberately excludes a browser rerun, runtime/deploy work, whole-project typecheck, other actions, Move/Block, and denials.

## Known gaps (current cycle)

- [ ] Full Move dependency repair, source Block parity, file-mutation authorization enforcement, and action-specific denials remain unverified/incomplete. Source and Lock mutations and authorization remain unproven. The styled Move/Layout/Delete/Native Restore roundtrip and the complete Files lifecycle are bounded disposable-fixture proof, not full dependency or authorization proof. The separate Delete-dialog acceptance is intercepted; it is not delete proof. Native Restore is proved only for its selected existing-control recovery scope. Clear-then-Save is not separately proved beyond Parent’s two-value save/readback/restore. Parent authorization has isolated 9/9 proof only; deployment/browser proof remains absent.
- [ ] The `3fa1838a3` Layout gate is bounded, not clean: Rust format/check and 4/4 isolated tests pass, but the test-only `6f3d99463` warning cleanup needs its targeted recheck. Scoped Prettier and ESLint pass; the current frontend gate remains pending. This is not full-replica parity evidence.

## Out of scope

- Move dependency-repair choices and source Block semantics are not recreated as Lock behavior.
- Wikidot's single-parent source model is not substituted for Cobalt's existing plural-parent model.
- Legacy upload-storage metadata unavailable from current file responses is not invented.
- No production deployment, source-site writes, source-editor side effects, imported rollback, or unrelated backend/non-Wikidot changes are authorized by this contract.
