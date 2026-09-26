# Cobalt page parents

The Parent panel edits page breadcrumb relationships in `framerail/src/routes/[slug]/[...extra]/ParentPane.svelte`. Unlike Wikidot’s single-parent form, Cobalt retains its existing plural, space-separated parent contract.

## What it must do

- [x] Explain breadcrumb relationships and space-separated multiple parent names in the Wikidot layout; label the input “Parent page names”.
- [x] Offer a Clear parents button that empties the input without submitting; keep Cancel and Save.
- [x] Look up the last typed slug token only after two characters, using the existing site- and viewer-scoped editor page lookup; suggestions retain preceding parent names.
- [x] Ignore stale lookup results and surface current lookup failures without preventing manual entry.
- [x] Save two real parent values, read them back, and restore the original parent set without replacing Cobalt’s plural write with a single-parent write.
- [ ] Save a cleared parent set remains unproven.

## How it works

- [Page actions](cobalt-page-actions.md) describes other bottom-page controls.

## Implementation inventory

- `framerail/src/routes/[slug]/[...extra]/ParentPane.svelte` — parent form, token suggestions, and plural update.
- `framerail/src/lib/editor-lookup.ts` — existing editor page lookup client.
- `framerail/src/lib/server/load/editor-lookup.ts` — trusted-context editor page lookup action.

## Tests asserting this spec

- `framerail/tests/page-parent-pane.test.ts` — SSR affordances, plural token completion, stale and failed lookup behavior.
- `framerail/tests/editor-lookup.test.ts` — trusted site/viewer request context and lookup failure.
- `framerail/tests/local/remaining-actions.mjs` — local lookup, two-parent save/readback, and restore.

## Known gaps (current cycle)

- [ ] Clear-then-save, authorization denial, and final independent checks remain pending. The bounded preservation record excludes only this sacrificial fixture, the Tags fixture, and native cache fields.

## Out of scope

- Replacing Cobalt’s plural parent contract with Wikidot’s single-parent model.
- New lookup backend or generic autocomplete framework.
- Browser, database, deployment, and page shell changes outside the proved local fixture.
