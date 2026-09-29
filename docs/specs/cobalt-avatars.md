# Cobalt avatars

Members have a round avatar: their uploaded image, or their initials. Imported members (see [replica status](../wiki/systems/cobalt-replica-status.md)) can start with the avatar they had on Wikidot.

## What it must do

- [x] An avatar is served at `/-/avatar/<user_id>` on the site's domain by WWS (the gateway routes `/-/avatar/` there); a user without one gets 404. The stored blob is `user.avatar_s3_hash`, set through `user_edit`'s `avatar_uploaded_blob_id` (the upload's own user only; Deepwell's `maximum-avatar-size-kb`, 250 KiB). `null` removes it.
- [x] `UserAvatar` shows the image, or up to two initials on the accent gradient. Callers with the hash pass it (no request when empty; the first 6 bytes version the URL so a change is never served stale). Lists that only know the ID request the image and show initials on 404.
- [x] Header, signed in: one button with the avatar, name and chevron opens the account menu (My account, Site members for site admins, Sign out). The name's dark panel starts at the avatar's vertical midline and is exactly the avatar's height (30 px). Signed out is unchanged.
- [x] My account has an Avatar form: current avatar, image picker, preview, Upload and (when set) Remove. The browser crops the chosen image to a centered square and encodes it as 256×256 WebP before upload, so phone photos fit the 250 KiB limit. Non-images are refused before any backend call; a failed storage upload leaves the avatar unchanged and says so in the Avatar section.
- [x] Avatars precede user names in the page history table (rows with a local account only; `Wikidot ID …` placeholders and Unknown stay plain), the members list and membership applications.
- [x] The import tool gives a member their Wikidot avatar only when their Wikidot profile (`user:info/<slug>`) reports their Cobalt user ID: locally created accounts also have positive IDs, which can belong to unrelated Wikidot users. It skips members who already have an avatar, Wikidot's `default_avatar`, non-images and files over 250 KiB, and uploads as the member. Dry run by default; `--apply` writes at most one member per `--interval` seconds (default 1). Reruns change nothing.
- [ ] Other places show only a raw user ID or name string and have no avatar: history pane, file revisions, locks, votes, tag admin log, watching activity. `[[*user name]]` in page wikitext renders FTML's placeholder avatar (`Handle::get_user_info` returns dummy users).

## Import

Wikidot's largest avatar is `size=normal` (about 100×100; `medium`/`large` return 403). At the local full import (31 members): 5 custom avatars, 25 Wikidot defaults, 1 slug with no Wikidot account.

1. Dry run: `python -m tools.cobalt_migration.wikidot_avatars http://127.0.0.1:27471/jsonrpc SITE_ID PASSWORD_FILE` (signs in as `cobalt-import`, a site admin, to list members).
2. Apply: the same command with `--apply`. Production storage is Cloudflare R2, so its HTTPS presign URLs are reachable through the SSH-forwarded endpoint.

Production, verified 2026-09-29 UTC: `--apply` imported booriley22, etone, invisibleinkie, ladystoryteller and luridel (12–21 KB PNGs, each served 200 at `/-/avatar/<id>` with its original size); 25 members kept Wikidot's default and were skipped; the rerun planned 0 (`/tmp/claude/avatars-prod-apply.log`, `avatars-prod-rerun.log`).

## Implementation inventory

- `framerail/src/lib/component/UserAvatar.svelte`, `framerail/src/lib/css/user-link.scss`.
- `framerail/src/lib/component/LoginStatus.svelte`, `framerail/src/lib/sigma-esque/wikidot.svelte` — header menu.
- `framerail/src/lib/avatar-resize.ts`, `framerail/src/routes/[x+2d]/settings/`, `framerail/src/lib/server/load/settings.ts`, `framerail/src/lib/server/deepwell/user.ts` — upload, removal, `AvatarUploadError`.
- `framerail/src/routes/[slug]/[...extra]/HistoryTable.svelte`, `framerail/src/routes/[x+2d]/admin/members/+page.svelte`.
- `tools/cobalt_migration/wikidot_avatars.py`.
- Existing: `wws/src/handler/avatar.rs`, `deepwell/src/services/user/service.rs` (`avatar_uploaded_blob_id`, size check, cache clear).

## Tests asserting this spec

- `framerail/tests/settings.test.ts`: upload (blob, PUT, `user_edit`), removal (`avatar_uploaded_blob_id: null`), non-image refusal, failed PUT, form render; 17/17 at `a17317801`.
- `framerail/tests/history-table.test.ts`, `framerail/tests/members.test.ts`: avatar markup before local names only; 17/17 at `49e0faec0`.
- `framerail/tests/local/avatar.mjs`: real browser upload of a 1200×800 PNG, resized WebP under 250 KiB, header image, served 256×256, removal to initials and 404; 1/1 locally.
- `framerail/tests/local/login.mjs`: header menu opens and contains My account; 1/1 locally.
- `tests/cobalt_migration/test_wikidot_avatars.py`: import, dry run, default avatar, existing/ID-less members, ID mismatch, missing avatar, non-image/oversized, order; 8/8 at `2288d83a3`. Local `--apply` imported 4 (plus 1 already set); rerun planned 0.
