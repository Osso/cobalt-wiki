# Cobalt table-of-contents styling

Wikidot-layout pages style the FTML-generated table of contents using the source Wikidot panel presentation. Framerail's `wikidot.svelte` maps the legacy base stylesheet's `#toc`, `#toc-action-bar`, and `#toc-list` rules to FTML's `wj-` IDs. The roster's existing fixed-position wrapper remains responsible for floating placement; no page source is rewritten. See [DOM compatibility](../dom-compatibility.md).

## What it must do

- [x] Render an opaque `#f7f7f7` panel with a one-pixel dashed gray border, table display, `0 0.5em` padding, and original `1em` vertical margins, matching the original Wikidot base stylesheet.
- [x] Center the bold title with original `0 2em` padding and `0.2em 0` margin; retain `0.5em 0` list margin and two-pixel action-bar padding.
- [x] Present plain roster links without list markers, with one-em UL padding and zero UL margin, matching Wikidot's indented link blocks.
- [x] Preserve the generated roster fixture's floating wrapper while scrolling and its 19 ordered player links.
- [x] Preserve every roster link's matching heading target; all 19 clicks select the correct fragment and bring the target into the viewport.
- [x] Leave Classic/Current tab palettes, account selections, page content, and layout outside the TOC unchanged.

## How it works

- [Legacy layout compatibility](../dom-compatibility.md).

## Implementation inventory

- `framerail/src/lib/sigma-esque/wikidot.svelte` — maps original TOC styles to compiled FTML IDs.
- `deepwell/vendor/ftml/src/render/html/element/toc.rs` — existing generated markup, unchanged by this stylesheet repair.

## Tests asserting this spec

- `framerail/tests/toc-style.test.mjs` — Chromium computed panel/title/list/action-bar styles, fixed placement after scroll, ordered links and anchor navigation.
- `framerail/tests/fixtures/roster-toc.html` — actual FTML TOC captured from the deployed roster, with its original floating wrapper and heading targets.
- `framerail/tests/color-profiles.test.mjs` — existing palette and content-preservation regression, rerun because the shared layout CSS changed.

Final source: `2c514d943a53a48b16a76fcb151d7280c4715639` (plain UL styling, configured CSS property order, and companion test typing fix). Saved regression `/tmp/cobalt-toc-final-green.log` passes 2/2 (TOC and profile/content preservation). Final full-margin/list regression `/tmp/cobalt-toc-margin-and-list-green.log` passes 1/1 and supersedes earlier TOC proof; it is not an additional unique test. Appearance companion `/tmp/cobalt-toc-settings-typed-green.log` passes 1/1.

Framerail deployed through `install/dev-deploy.sh framerail`, exit 0, at that revision (`/tmp/cobalt-toc-final-framerail-deploy.log`). Final production acceptance `/tmp/cobalt-toc-final-live-proof.log` passes: HTTP 200; exact source panel/title computed-style match; no UL markers, one-em indent, zero UL margin; all 19 links clicked with correct fragments and targets in viewport; fixed position after scrolling. Exact TOC markup and all 19 heading HTML remain unchanged against baseline; Current palette remains unchanged. Screenshot: `/tmp/cobalt-toc-final-live.png`. This supersedes the earlier representative-Luridel-only receipt.

Account receipt `/tmp/cobalt-toc-preference-preservation.json` confirms `luridel` (`121356`) remains Classic with unrelated fields unchanged. That read is stamped `e39d9f311`; final CSS/test-only changes leave the account backend untouched, so it remains applicable, not a new final-revision account read.

Independent agent 12's final four scoped gates pass at `2c514d943`: Svelte check 0 errors/0 warnings, Stylelint 0 diagnostics, ESLint and Prettier exit 0. Exact receipts: `/tmp/cobalt-toc-final-verify-svelte-check.log`, `/tmp/cobalt-toc-final-verify-eslint.log`, `/tmp/cobalt-toc-final-verify-prettier.log`, `/tmp/cobalt-toc-final-verify-stylelint.log`. Consolidated report: `/tmp/cobalt-toc-verifier-report.md`; revision/hash ledger: `/tmp/cobalt-toc-final-verifier-proof.json`. Existing deploy build notices and Nix cache notices are not claimed resolved.

## Known gaps (current cycle)

None for the original-goal TOC styling repair. Evidence establishes bounded roster styling, preservation, scrolling, and all-link acceptance, not full pixel parity or whole-replica readiness.

## Out of scope

- New layouts, dark mode, account/data mutations, and changes to the TOC generator.
- FTML's existing unlabeled fold-control stub: this repair maps missing visual rules; it does not implement new fold/unfold controls or reproduce the source's extra table wrappers/Fold row. No full pixel parity or whole-replica readiness is claimed.
