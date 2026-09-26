# Cobalt page actions

Bottom-page controls follow the supplied Tags/History screenshots and retained Wikidot behavior. This contract covers the Wikidot layout; [history listing](cobalt-history-listing.md) and [history import](cobalt-history-import.md) define history data and provenance.

## What it must do

### Bottom actions

- [x] Render Edit, Tags, History, Files, and More options.
- [x] Omit Vote in the Wikidot layout; preserve voting and Tags controls in other layouts.
- [x] Provide separate imported/local History datasets, seven filters, five page sizes, numbered pagination, source, read-only preview, and same-origin comparison.
- [ ] Backlinks and Print remain absent. Other wired panels require separate second-pass verification.

### Tags

- [x] Render Page Tags, explanatory links, labeled form-table input, space-separated hint, and close/clear/save controls.
- [x] Display Wikidot tags sorted; retain set-based additions/removals rather than promise backend array order.
- [x] Clear empties only the input; close discards unsaved input. Neither sends tag changes.
- [x] Save submits the exact additions/removals. The exercised authorized fixture saves, reads back, and restores its original tag set without changing page source.
- [ ] Independently exercise denied tag-save authorization; successful authorized fixture testing does not prove denial behavior.

## Evidence

- `framerail/tests/page-bottom-tags.test.ts`: five SSR cases cover Wikidot markup, sorted input, alternate-layout preservation, Vote omission, and retained bottom actions.
- `framerail/tests/local/page-bottom-actions.mjs`: real browser clear/close/reopen/save/readback/restore with a fixture-only write guard. Final pass: `/tmp/claude/cobalt-bottom-tags-bounded-browser.log`.
- `framerail/tests/local/history-actions.mjs`: root `home:_public` has 57 imported records; named `home:start` has 240 imported and two native records. Exercises seven filters, all five sizes, a disjoint second page, source/preview/compare and dataset selection. Final pass: `/tmp/claude/cobalt-bottom-history-final-browser.log`.
- Imported metadata (`M`) has no live records; the browser proves an empty result, while isolated backend fixtures prove positive metadata/tag distinction. No historical metadata is fabricated.
- Independent gates: `/tmp/claude/cobalt-bottom-backend-final-gate.json` and `/tmp/claude/cobalt-bottom-frontend-final-gate.json`.

A Tags test run stalled and was terminated; all owned processes exited and fixture tags were restored. The identical-code bounded retry passed. The stall's cause remains unproven; no application fix or flake-free claim follows from that retry.

## Preservation and deployment

The local runtime executable matches the built Deepwell hash (`/tmp/claude/cobalt-bottom-local-runtime.json`). Main preservation checks retain identical hashes for 6,116 pages, 45,369 imported records, 10,091 native-content rows, 32 grants and zero drafts. Only the sacrificial Tags fixture and native renderer-cache fields are excluded (`/tmp/claude/cobalt-bottom-preservation-result.json`).

Local screenshots were compared with the supplied references: `/tmp/claude/cobalt-history-panel-final.jpg` and `/tmp/claude/cobalt-tags-panel-local.png`. This is scoped visual evidence, not pixel-perfect parity.

## Remaining scope

Files, More options, Parent, Source, Move, Delete and Lock are wired but lack complete second-pass interaction/permission proof. Backlinks and Print are absent. This slice does not establish all-panel or full-replica parity.

No production deployment, source-site writes, source-editor side effects, imported rollback, or unrelated backend/non-Wikidot changes are authorized by this contract.
