# Cobalt page parents

The Parent panel edits page breadcrumb relationships in `framerail/src/routes/[slug]/[...extra]/ParentPane.svelte`. Unlike Wikidot’s single-parent form, Cobalt retains its existing plural, space-separated parent contract.

## What it must do

- [x] Explain breadcrumb relationships and space-separated multiple parent names in the Wikidot layout; label the input “Parent page names”.
- [x] Keep input, Clear and Save disabled until existing parent names load; show loading or failure explicitly and keep Cancel available. Never overwrite an editable draft with the initial response.
- [x] Offer a Clear parents button that empties the input without submitting; keep Cancel and Save.
- [x] Look up the last typed slug token only after two characters, using the existing site- and viewer-scoped editor page lookup; suggestions retain preceding parent names.
- [x] Ignore stale lookup results and surface current lookup failures without preventing manual entry.
- [x] Save two real parent values, read them back, and restore the original parent set after a controlled delayed baseline load, without replacing Cobalt’s plural write with a single-parent write.
- [x] Authorize a parent update against the actual request actor and site, then the body-selected child page’s `Page/Edit` permission; never borrow the header page’s permission.
- [ ] Clear-then-Save is not separately proved.

## How it works

- [Page actions](cobalt-page-actions.md) describes other bottom-page controls.

## Implementation inventory

- `framerail/src/routes/[slug]/[...extra]/ParentPane.svelte` — parent form, token suggestions, and plural update.
- `framerail/src/lib/editor-lookup.ts` — existing editor page lookup client.
- `framerail/src/lib/server/load/editor-lookup.ts` — trusted-context editor page lookup action.

## Tests asserting this spec

- `framerail/tests/page-parent-pane.test.ts` — SSR affordances, plural token completion, stale and failed lookup behavior.
- `framerail/tests/editor-lookup.test.ts` — trusted site/viewer request context and lookup failure.
- `framerail/tests/local/remaining-actions.mjs` — controlled delayed-GET local lookup, two-parent save/readback, and restore.

## Evidence

- `/tmp/claude/cobalt-parent-loading-red.json` — pre-baseline editable-input race reproduced (`typedRetained: false`, `afterEmpty: true`); `71ce6afa2` disables editing until the baseline loads.
- `/tmp/claude/cobalt-remaining-actions-parent-ready.log` — controlled delayed-GET local browser pass (1/1) for lookup, two-parent save/readback/restore, and fixture restoration.
- `/tmp/claude/cobalt-parent-final.png` — local captured parent state; presentation evidence only.
- `ba73f942f` — binds parent updates to the request actor/site and checks `Page/Edit` for the body child. The isolated `deepwell/tests/page_layout_permission.rs` coverage passed 9/9 for allowed plural updates, denied actor, forged actor, site mismatch, and header/body target separation. It has not been deployed or exercised in a browser.
- `/tmp/claude/cobalt-action-preservation-gate.json` — read-only preservation PASS for its bounded non-Files streams; it excludes three page identities, five native cache fields, Files, and 43 unreconstructable original full-row fingerprints.

## Known gaps (current cycle)

- [ ] Clear-then-Save remains unproved as a separate mutation; two-parent save/readback/restore is proved. Parent authorization has isolated 9/9 coverage only; deployment and browser proof remain absent. The independent frontend gate passes; see [page actions](cobalt-page-actions.md). The newer bounded preservation PASS excludes three page identities, five native cache fields, Files, and 43 unreconstructable original full-row fingerprints.

## Out of scope

- Replacing Cobalt’s plural parent contract with Wikidot’s single-parent model.
- New lookup backend or generic autocomplete framework.
- Browser, database, deployment, and page shell changes outside the proved local fixture.
- Claiming a cleared-parent write from the proved two-value save/readback/restore path.
