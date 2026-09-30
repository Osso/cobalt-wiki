# Cobalt account color profiles

Cobalt members choose Classic or Current navigation colors in My account → Appearance. The preference belongs to the account, not the browser. It does not select a light/dark theme or change layout. Account editing follows the existing [account settings](cobalt-accounts.md) path.

## What it must do

- [ ] Existing and new accounts start with Current, preserving the existing appearance; anonymous visitors also see Current.
- [ ] Classic uses cyan navigation tabs (`#a8d7ea`), pale cyan hover (`#d6f3ff`), and black labels (`#000`). Dropdowns use the same palette.
- [ ] Current keeps tab fill `#bcc9f2` and hover `#e0e6fb`; labels use slightly darker navy `#0b1a38` instead of `#10254f`.
- [ ] Saving Classic or Current persists across page reloads and devices signed into that account; unrelated account edits preserve the choice.
- [ ] Appearance saves use the signed-in account, ignoring submitted account IDs. Signed-out saves and unsupported values are rejected without changing preferences.
- [ ] The selected colors apply to the first rendered page and update after saving; geometry, corner radii, navigation content, and page content remain unchanged.
- [ ] Appearance remains private account data, not another member's public profile data.

## How it works

- [Account profile design](../plans/2026-09-30-cobalt-color-profiles.md).

## Implementation inventory

- `deepwell/migrations/` and `deepwell/src/models/user.rs` — durable account preference and Current default.
- `deepwell/src/services/user/` — validation and existing user-edit persistence.
- `framerail/src/lib/types.ts`, `framerail/src/lib/user-data.ts` — account-only preference handoff.
- `framerail/src/lib/server/deepwell/user.ts`, `framerail/src/lib/server/load/settings.ts`, `framerail/src/routes/[x+2d]/settings/` — authenticated appearance form and save.
- `framerail/src/routes/+layout.svelte`, `framerail/src/lib/sigma-esque/wikidot.svelte` — server-selected navigation palette.

## Tests asserting this spec

- `deepwell/tests/user_color_profile.rs` — defaults, edit/readback across transactions, invalid values, unrelated edits, other accounts unchanged.
- `framerail/tests/settings.test.ts` — saved choice display, authenticated save, invalid input, anonymous rejection.
- `framerail/tests/user-data.test.ts` — private preference handoff.
- `framerail/tests/color-profiles.test.mjs` — rendered tab/dropdown/hover colors and unchanged geometry.

## Known gaps (current cycle)

- [ ] Verify durable backend persistence and integrated account rendering.
- [ ] Deploy bounded profile support and confirm Luridel's requested Classic selection.

## Out of scope

- Light/dark/system themes, custom palettes, and layout profiles: not requested.
- TOC styling parity and content-tab colors: separate from header navigation profiles.
