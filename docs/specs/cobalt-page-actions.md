# Cobalt page actions

Cobalt page actions render the bottom-page controls and the Wiki-only Tags pane. The source contract is bounded to the supplied second-pass screenshots and legacy Wikidot reference; `bfd4e5c38` and its SSR tests prove rendered presence only. [History listing](cobalt-history-listing.md) and [history import](cobalt-history-import.md) govern distinct history work.

## What it must do

### Bottom actions

- [x] In the Wiki layout, render Edit, Tags, History, Files, and More options in the bottom action inventory.
- [x] Omit Vote from the Wiki layout. Vote is not required in Cobalt's Wiki layout.
- [x] Preserve the existing Vote control and Tags layout outside the Wiki layout.
- [ ] Dispatch each retained Wiki bottom action to its live pane and verify it in the main browser.

### Tags

- [x] In the Wiki layout, render the legacy Page Tags heading, explanatory links, form table, `size="50"` tags input, space-separated hint, and close, clear, and save tags controls.
- [ ] Clear only empties the Tags input before submission; it does not save or change stored tags. Main-browser interaction proof pending.
- [ ] Close dismisses the Tags pane without saving; main-browser interaction proof pending.
- [ ] Save submits only permissioned tag changes; authorization, persistence, and readback proof pending.

### Evidence boundaries

- [x] SSR proves the scoped Wiki markup and action inventory, plus preserved alternate-layout controls, in four tests.
- [ ] Main-browser interaction and visual confirmation against the supplied reference remain pending. No pixel-parity claim follows from SSR.

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

- [ ] Main browser: exercise close, clear, and save; verify clear/close make no write and save has permissioned readback.
- [ ] Main browser: compare the scoped controls visually with the supplied second-pass reference.
- [ ] History UI remains owned by the in-progress [history listing](cobalt-history-listing.md) work.

## Out of scope

- History API shape, revision metadata, fake version/compare/rollback controls, and unknown metadata: owned by history work or unverified.
- Source-editor access, production deployment, source-site writes, and pixel-parity claims: no evidence or authorization in this scope.
- Backend tag behavior and non-Wiki layouts, except preserving their existing controls.
