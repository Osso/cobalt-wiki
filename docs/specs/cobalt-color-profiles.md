# Cobalt account color profiles

Cobalt members choose Classic or Current navigation colors in My account → Appearance. The preference belongs to the account, not the browser. It does not select a light/dark theme or change layout. Source lives in Deepwell's user model/services and Framerail's account settings/Wikidot layout. Account editing follows the existing [account settings](cobalt-accounts.md) path; deployment evidence and limits live in the [profile plan](../plans/2026-09-30-cobalt-color-profiles.md#deployment-and-verification).

verified: 2026-09-30

## What it must do

- [x] Existing and new accounts start with Current; anonymous visitors also see Current. Existing backgrounds remain unchanged, with the explicitly requested darker labels below.
- [x] Signed-in members can find the selector in My account → Appearance (`/-/settings#appearance`), choose Current or Classic under Tab colors, and submit Save appearance. The saved choice is displayed when reopening settings.
- [x] Classic uses cyan navigation tabs (`#a8d7ea`), pale cyan hover (`#d6f3ff`), and black labels (`#000`). Dropdowns use the same palette.
- [x] Current keeps tab fill `#bcc9f2` and hover `#e0e6fb`; labels use slightly darker navy `#0b1a38` instead of `#10254f`.
- [x] Saving Classic or Current persists across page reloads and devices signed into that account; unrelated account edits preserve the choice.
- [x] Appearance saves use the signed-in account, ignoring submitted account IDs. Signed-out saves and unsupported values are rejected without changing preferences.
- [x] The selected colors apply to the first rendered page and update after saving; tab width, height, padding, and corner radii remain unchanged.
- [x] Navigation content and page content remain unchanged; the rendered fixture is identical across profiles apart from the selected profile attribute.
- [x] Appearance remains private account data, not another member's public profile data.

## How it works

- [Account profile design](../plans/2026-09-30-cobalt-color-profiles.md).

## Implementation inventory

- `deepwell/migrations/20260930000000_user_color_profile.sql` — durable Current default and allowed-value constraint.
- `deepwell/src/types/enums/color_profile.rs`, `deepwell/src/types/enums/mod.rs` — Classic/Current enum and export.
- `deepwell/src/models/user.rs` — persisted account preference.
- `deepwell/src/services/user/service.rs`, `deepwell/src/services/user/structs.rs`, `deepwell/src/services/audit/structs.rs` — user-edit persistence, typed input/serialization, and audit field.
- `framerail/src/app.d.ts` — account preference declaration for page data.
- `framerail/src/lib/types.ts`, `framerail/src/lib/user-data.ts` — account-only preference handoff.
- `framerail/src/lib/server/deepwell/user.ts`, `framerail/src/lib/server/load/settings.ts`, `framerail/src/routes/[x+2d]/settings/` — authenticated appearance form and save.
- `framerail/src/routes/+layout.svelte`, `framerail/src/lib/sigma-esque/wikidot.svelte` — server-selected navigation palette.

## Tests asserting this spec

- `deepwell/tests/user_color_profile.rs` — defaults, edit/readback across transactions, invalid values, unrelated edits, other accounts unchanged.
- `framerail/tests/settings.test.ts` — saved choice display, authenticated save, invalid input, anonymous rejection.
- `framerail/tests/user-data.test.ts` — private preference handoff.
- `framerail/tests/color-profiles.test.mjs` — rendered tab/dropdown/hover colors, anonymous Current default, unchanged tab dimensions/padding/radii, and identical navigation/page content.
- `deepwell/src/services/user/structs.rs` (`user_serialization`) — native Current/Classic serialized values and unchanged Wikidot fixture shape.

Tracked regression results: frontend 27/27 (23 settings, 3 sanitizer, 1 component), durable database/input 2/2, serialization 1/1. The final component rerun passed 1/1 including content-preservation assertions (`/tmp/cobalt-color-profiles-content-preservation-green.log`); it replaces the earlier component proof and is not a 28th frontend test.

One-off authenticated local full-stack acceptance (`/tmp/cobalt-profile-local-smoke.mjs`, output `/tmp/cobalt-color-profiles-authenticated-smoke-diagnostic.log`) additionally proves UI save/reload, raw browser-navigation SSR, a second browser/login retaining Classic, both computed palettes, and equal geometry. It is not a tracked automated regression. Exact artifacts, source/deployed revisions, and production limits are recorded in the [deployment receipt](../plans/2026-09-30-cobalt-color-profiles.md#deployment-and-verification).

## Known gaps (current cycle)

No remaining gap in this bounded profile change. Durable persistence, local authenticated rendering, deployment, and Luridel's saved selection are evidenced in the [deployment receipt](../plans/2026-09-30-cobalt-color-profiles.md#deployment-and-verification). Production authenticated Luridel rendering was not directly observed.

## Out of scope

- Light/dark/system themes, custom palettes, and layout profiles: not requested.
- TOC styling parity and content-tab colors: separate from header navigation profiles.
