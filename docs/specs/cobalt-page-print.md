# Cobalt page print view

The print view at `/printer--friendly/<slug>` presents an existing page without site navigation or editing controls. Its route and layout live in `framerail/src/routes/`.

## What it must do

- [x] Render the page revision title, compiled body, source-page URL, and an explicit print button; never open the print dialog on load.
- [x] Render an existing translated license notice when present, without inventing one when absent.
- [x] Suppress standard site header, sidebar, footer, and bottom page actions while preserving root layout context.
- [x] Use the normal page view with site, locale, and session authorization; deny missing and unauthorized pages.
- [x] Resolve `/printer--friendly/` through the site's existing default-page behavior.
- [x] Return the same rendered DOM for named and root pages apart from framework marker comments; return `404` for a missing page.
- [x] Register no route actions, forms, or editor locks.
- [ ] Open the page's canonical print link in a new tab. The bottom-page control is owned separately.

## How it works

- [Page metadata](cobalt-page-metadata.md)
- [Page actions](cobalt-page-actions.md) defines bottom-action registration.

## Implementation inventory

- `framerail/src/routes/printer--friendly/[...path]/+page.server.ts` — loads the page through the normal page-view path.
- `framerail/src/routes/printer--friendly/[...path]/+page.svelte` — renders print-only content and control.
- `framerail/src/routes/+layout.svelte` — omits standard chrome for print-view data.

## Tests asserting this spec

- `framerail/tests/page-print.test.ts`
- `framerail/tests/local/remaining-actions.mjs` — local named/root DOM comparison and missing-page `404`.

## Known gaps (current cycle)

- [ ] Confirm bottom-page print-link target/new-tab contract in separately owned registration.
- [ ] Final independent checks remain ongoing; the bounded preservation record excludes only the sacrificial Tags/Parent fixture and native cache fields.

## Out of scope

- New page-view permissions, license synthesis, editor actions, and automatic printing.
- Production deployment or source-site changes.
