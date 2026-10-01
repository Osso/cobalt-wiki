# Cobalt table-of-contents styling

Wikidot-layout pages style the FTML-generated table of contents using the source Wikidot panel presentation. Framerail's `wikidot.svelte` maps the legacy base stylesheet's `#toc`, `#toc-action-bar`, and `#toc-list` rules to FTML's `wj-` IDs. The roster's existing fixed-position wrapper remains responsible for floating placement; no page source is rewritten. See [DOM compatibility](../dom-compatibility.md).

## What it must do

- [ ] Render an opaque `#f7f7f7` panel with a one-pixel dashed gray border, table display, `0 0.5em` padding, and `1em auto 1em 0` margin, matching the original Wikidot base stylesheet.
- [ ] Center the bold title with original `0 2em` padding and `0.2em 0` margin; retain `0.5em 0` list margin and two-pixel action-bar padding.
- [ ] Preserve the roster's floating wrapper while scrolling, its 19 ordered player links, and navigation to the matching heading anchors.
- [ ] Leave Classic/Current tab palettes, account selections, page content, and layout outside the TOC unchanged.

## How it works

- [Legacy layout compatibility](../dom-compatibility.md).

## Implementation inventory

- `framerail/src/lib/sigma-esque/wikidot.svelte` — maps original TOC styles to compiled FTML IDs.
- `deepwell/vendor/ftml/src/render/html/element/toc.rs` — existing generated markup, unchanged by this stylesheet repair.

## Tests asserting this spec

- `framerail/tests/toc-style.test.mjs` — Chromium computed panel/title/list/action-bar styles, fixed placement after scroll, ordered links and anchor navigation.
- `framerail/tests/fixtures/roster-toc.html` — actual FTML TOC captured from the deployed roster, with its original floating wrapper and heading targets.
- `framerail/tests/color-profiles.test.mjs` — existing palette and content-preservation regression, rerun because the shared layout CSS changed.

## Known gaps (current cycle)

- [ ] Deploy corrected Framerail and confirm real roster appearance/anchor behavior.

## Out of scope

- New layouts, dark mode, account/data mutations, and changes to the TOC generator.
- FTML's existing unlabeled fold-control stub: this repair maps missing visual rules; it does not implement new fold/unfold controls.
