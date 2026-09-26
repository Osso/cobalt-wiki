# Cobalt page actions

Cobalt page actions render the bottom-page controls and the Wiki-only Tags pane. The source contract is bounded to the supplied second-pass screenshots and legacy Wikidot reference; `bfd4e5c38` and its SSR tests prove rendered presence only. [History listing](cobalt-history-listing.md) and [history import](cobalt-history-import.md) govern distinct history work.

## What it must do

### Bottom actions

- [x] In the Wiki layout, render Edit, Tags, History, Files, and More options in the bottom action inventory.
- [x] Omit Vote from the Wiki layout. Vote is not required in Cobalt's Wiki layout.
- [x] Preserve the existing Vote control and Tags layout outside the Wiki layout.
- [x] Exercise History in the main local browser for root `home:_public` (57 rows), named `home:start` (240 rows), and two native rows. The exercised page uses seven filters, five page sizes, a second page, and separate source, read-only render, and same-origin compare controls.
- [ ] Backlinks and Print remain absent. Files, + Options, Parent, Source, Move, Delete, and Lock remain uncertified in the second-pass browser.

### Tags

- [x] In the Wiki layout, render the legacy Page Tags heading, explanatory links, form table, `size="50"` tags input, space-separated hint, and close, clear, and save tags controls.
- [x] Sort Wikidot Tags before rendering and saving them.
- [x] Save and restore the exercised Tags set in the local browser workflow (1/1).
- [ ] Clear only empties the Tags input before submission; it does not save or change stored tags. Main-browser interaction proof pending.
- [ ] Close dismisses the Tags pane without saving; main-browser interaction proof pending.
- [ ] Save submits only permissioned tag changes; authorization and broader persistence/readback proof pending.

### Evidence boundaries

- [x] SSR proves the scoped Wiki markup and action inventory, plus preserved alternate-layout controls, in four tests.
- [x] `/tmp/claude/cobalt-bottom-tags-save-green.log` records one local Tags save/restore browser pass.
- [x] `/tmp/claude/cobalt-history-actions-postdeploy.log` records one local History browser pass after `3537f8435`: root 57, named 240, and native 2 rows.
- [x] `/tmp/claude/cobalt-bottom-backend-final-gate.json` independently passes backend authorization, origin, read-only, pagination, and mapping evidence for the History listing refactor.
- [x] `/tmp/claude/cobalt-bottom-local-runtime.json` binds the running main local runtime hash to the built Deepwell executable.
- [x] `/tmp/claude/cobalt-bottom-preservation-result.json` preserves identical hashes for 6,116 pages, 45,369 history rows, 10,091 native-content rows, zero drafts, and 32 grants; it excludes only the sacrificial Tags fixture and native renderer-cache fields.
- [x] Final local captures compare the main History panel with the supplied source and capture the local Tags panel (`/tmp/claude/cobalt-history-panel-final.jpg`, `/tmp/claude/cobalt-tags-panel-local.png`). This is scoped visual evidence, not pixel-perfect or full-replica parity.
- [ ] Close/clear behavior, permissioned Tags readback, and second-pass verification of other wired action panels remain incomplete. Do not infer all-panel or pixel parity from the scoped evidence.

## How it works

- [Page actions system](../wiki/systems/cobalt-page-actions.md) (documentation stub).
- [History listing](cobalt-history-listing.md) — separate in-progress history UI and read contract.
- [History import](cobalt-history-import.md) — separate preserved-source history contract.

## Implementation inventory

- `framerail/src/routes/[slug]/[...extra]/page.svelte` — selects bottom actions and page panes.
- `framerail/src/routes/[slug]/[...extra]/TagsPane.svelte` — renders and edits Tags controls.

## Tests asserting this spec

- `framerail/tests/page-bottom-tags.test.ts` — four SSR tests for Wiki Tags markup, alternate-layout preservation, Wiki Vote omission, and retained bottom actions.

## Known gaps (current cycle)

- [ ] Main browser: exercise close and clear; verify they make no write, and expand permissioned save/readback coverage.
- [ ] Main browser: verify the other wired action panels individually; Backlinks and Print are absent.
- [x] Frontend type check reports zero errors and warnings; style check reports zero issues. Four lint warnings were fixed at `de229645c`.
- [ ] Final scoped lint recheck remains pending; do not treat the earlier independent recheck as completed proof.
- [ ] History UI remains owned by the in-progress [history listing](cobalt-history-listing.md) work.

## Out of scope

- History API shape, revision metadata, fake version/compare/rollback controls, and unknown metadata: owned by history work or unverified.
- Source-editor access, production deployment, source-site writes, and pixel-parity claims: no evidence or authorization in this scope.
- Backend tag behavior and non-Wiki layouts, except preserving their existing controls.
