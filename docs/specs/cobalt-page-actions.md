# Cobalt page actions

Bottom-page controls follow supplied Tags/History screenshots and retained Wikidot behavior. This contract covers the Wikidot layout; [history listing](cobalt-history-listing.md) and [history import](cobalt-history-import.md) define history data and provenance.

## What it must do

### Bottom actions

- [x] Render Edit, Tags, History, Files, Backlinks, Print, and More options.
- [x] Omit Vote in the Wikidot layout; preserve voting and Tags controls in other layouts.
- [x] Provide separate imported/local History datasets, seven filters, five page sizes, numbered pagination, source, read-only preview, and same-origin comparison.
- [x] Open and close Source, Files, Parent, Move, Delete, Lock, and Layout panels in a local browser pass without approved writes.
- [x] Read Files, Parent, and Lock panel data without writes: `?/fileList`, `?/parentGet`, and `?/lockHistory` return `200`.
- [x] Show viewer-filtered Backlinks links and inclusions, including an empty inclusions result.
- [x] Require explicit Delete confirmation before its POST path; both acceptance and dismissal are exercised with every mutation intercepted, so no page is deleted.
- [ ] Opening panels does not establish source edits, moves, deletion, locks, layout saves, or their authorization.

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
- `framerail/tests/local/delete-confirmation.mjs` — Delete acceptance/dismissal with mutation interception.

## Evidence

- `/tmp/claude/cobalt-remaining-actions-browser.log` — local browser coverage of Source/options, passive Move/Delete/Lock/Layout panels, Files, Parent, Backlinks, and Print; 1/1 pass at `dbe0b29e0`.
- `/tmp/claude/cobalt-delete-confirmation-browser.log` — Delete acceptance/dismissal and mutation interception; 1/1 pass.
- `/tmp/claude/cobalt-remaining-preservation.json` — preserved: 6,116 original pages, 45,369 imported records, 10,091 native-content rows, 32 grants, and zero drafts. Excludes only the sacrificial Tags/Parent fixture and native cache fields.
- `/tmp/claude/cobalt-remaining-runtime.json` — running Deepwell SHA equals built `fc3e48773`.

## Known gaps (current cycle)

- [ ] Panel mutations and authorization remain unproven except the guarded Tags fixture.
- [ ] Final independent checks remain ongoing; this is not full-replica parity evidence.

## Out of scope

- Move dependency-repair choices and source Block semantics are not recreated as Lock behavior.
- Wikidot's single-parent source model is not substituted for Cobalt's existing plural-parent model.
- Legacy upload-storage metadata unavailable from current file responses is not invented.
- No production deployment, source-site writes, source-editor side effects, imported rollback, or unrelated backend/non-Wikidot changes are authorized by this contract.
