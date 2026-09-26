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
- [ ] Apart from guarded Tags and Parent save/readback/restore, panel mutations and authorization remain unproven. A disposable-page mutation run completed its first Move but failed its four blocked-foreign-GET assertion; error-free recovery is not lifecycle proof. Accepted Delete does not delete because its request is intercepted.

### Tags

- [x] Render Page Tags, explanatory links, labeled form-table input, space-separated hint, and close/clear/save controls.
- [x] Display Wikidot tags sorted; retain set-based additions/removals rather than promise backend array order.
- [x] Clear empties only the input; close discards unsaved input. Neither sends tag changes.
- [x] Save submits the exact additions/removals. The exercised authorized fixture saves, reads back, and restores its original tag set without changing page source.
- [ ] Independently exercise denied tag-save authorization; successful authorized fixture testing does not prove denial behavior.

## How it works

- [Page backlinks](cobalt-page-backlinks.md) defines the permission-filtered incoming-link read API.
- [Page files](cobalt-page-files.md), [page parents](cobalt-page-parent.md), and [page print view](cobalt-page-print.md) define their respective pane and route contracts.

## Implementation inventory

- `framerail/src/routes/[slug]/[...extra]/+page.svelte` — bottom action registration and panels.
- `framerail/src/routes/[slug]/[...extra]/FilePane.svelte` — Files list and information view.
- `framerail/src/routes/[slug]/[...extra]/ParentPane.svelte` — parent editor.

## Tests asserting this spec

- `framerail/tests/page-bottom-tags.test.ts` — SSR bottom-action and Tags behavior.
- `framerail/tests/local/page-bottom-actions.mjs` — Tags save/readback/restore with a fixture-only write guard.
- `framerail/tests/local/history-actions.mjs` — local History browser coverage.
- `framerail/tests/local/remaining-actions.mjs` — local panel, Files, Parent, Backlinks, and Print browser coverage.
- `framerail/tests/local/delete-confirmation.mjs` — native Delete dialog Cancel/Escape/acceptance with mutation interception; Move remains immediate.
- `framerail/tests/local/page-action-mutations.mjs` — disposable-page mutation lifecycle regression; its main run is not a pass.

## Evidence

- `/tmp/claude/cobalt-remaining-actions-parent-ready.log` — controlled delayed-GET local coverage; 1/1 pass for Source options/passive panels, parent lookup, two-parent save/readback/restore, and all remaining Files, Backlinks, and Print panels.
- `/tmp/claude/cobalt-delete-dialog-browser.log` — native Delete dialog Cancel/Escape/acceptance with the accepted DELETE intercepted; Move remains immediate; 1/1 pass at `931ca6f78`.
- `/tmp/claude/cobalt-page-action-mutations-browser.log` — main disposable-page run after `2e815055d`: first Move succeeded, then the four blocked foreign GET assertion failed; recovery returned without error. This is not a lifecycle pass.
- `/tmp/claude/cobalt-{delete-dialog,backlinks,print}-final.png` — local captures of the exercised Delete, Backlinks, and Print states; captures record presentation only, not mutation proof.
- `/tmp/claude/cobalt-parent-loading-red.json` — reproduced pre-baseline editable-input race (`typedRetained: false`, `afterEmpty: true`); `71ce6afa2` disables editing until the baseline loads.
- `/tmp/claude/cobalt-remaining-preservation.json` — preserved: 6,116 original pages, 45,369 imported records, 10,091 native-content rows, 32 grants, and zero drafts. This proof predates sacrificial pages `3000006134` and `3000006135`, so its original-page count does not cover those fixtures; it excludes the earlier sacrificial Tags/Parent fixture and native cache fields.
- `/tmp/claude/cobalt-remaining-runtime.json` — running Deepwell SHA equals built `fc3e48773`.

## Known gaps (current cycle)

- [ ] Full Move dependency repair, source Block parity, file mutations, and action-specific denials remain unverified/incomplete. Source, Lock, and Layout mutations and authorization also remain unproven. The disposable-page run did not complete its lifecycle after the blocked-foreign-GET assertion failure; recovery alone is not proof. Delete acceptance is intercepted; it is not delete proof. Clear-then-Save is not separately proved beyond Parent’s two-value save/readback/restore.
- [x] Independent frontend gate passes at `40714b041`: lint, format, style, type checks and scoped behavioral evidence; `/tmp/claude/cobalt-remaining-frontend-gate.json`. Backend gate passes independently. This is not full-replica parity evidence.

## Out of scope

- Move dependency-repair choices and source Block semantics are not recreated as Lock behavior.
- Wikidot's single-parent source model is not substituted for Cobalt's existing plural-parent model.
- Legacy upload-storage metadata unavailable from current file responses is not invented.
- No production deployment, source-site writes, source-editor side effects, imported rollback, or unrelated backend/non-Wikidot changes are authorized by this contract.
