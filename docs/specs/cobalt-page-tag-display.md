# Cobalt page tag display

Page tags render below page content in `framerail/src/routes/[slug]/[...extra]/page.svelte`.

## What it must do

- [x] Wikidot tag links remain atomic (one client rect per link), wrapping between links into multiple rows within the existing tag container and content width, including 2,000 unique tags totaling 35,999 characters.
- [x] At viewport widths 1440px and 390px, the tag container's scroll width does not exceed its client width by more than 1px. The fixed 1000px theme may still require horizontal page panning.
- [x] Every tag remains displayed with unchanged text and its existing same-origin `/system:page-tags/tag/<tag>` target. Wrapping changes no page source, labels, buttons, fonts, or tag-editor behavior.
- [x] Wikijump retains its existing wrapped tag list. Current and historical Wikidot tag displays share the wrapping behavior.

## How it works

- [Page actions](cobalt-page-actions.md) define surrounding controls.

## Implementation inventory

- `framerail/src/routes/[slug]/[...extra]/page.svelte` — current/historical tag rendering and display styles.

## Tests asserting this spec

- `framerail/tests/local/tag-wrap.mjs` — read-only retained `membersonly:tag-capacity-2000` fixture; 2,000 unique visible links, tag-string length, link identity, one client rect per link, multiple rows, preserved controls/fixed page width, and container overflow at 1440px/390px. Copies existing authorized CDP browser cookies in memory into its own isolated context; closes only that context and disconnects its transport. No login or fixture mutations. Run from `framerail/` with `COBALT_LOCAL_PREVIEW=<deployed-origin> node --test tests/local/tag-wrap.mjs` (CDP defaults to `http://127.0.0.1:9222`; override with `COBALT_CDP_ENDPOINT`). Screenshots: `/tmp/claude/tag-wrap-{1440,390}.png`.

## Known gaps (current cycle)

None. Deployed with Framerail `d0a27a049` (includes `9dfe9c629`); live GREEN 3/3 on 2026-10-14 at 1440px and 390px: 2,000 tags, 250 rows, zero fragmented links, 960px client/scroll width. Before the fix: 250 links split at hyphens per width.

## Out of scope

- Tag replacement backend, tag limits, source edits, theme responsiveness, fonts, images, privacy, login, and deployment orchestration.
