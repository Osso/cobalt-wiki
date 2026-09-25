# Cobalt Site Manager entry point

The imported Admin menu's `/_admin` entry must reach the native site administration screen. This is the existing site-info editor and [member administration](cobalt-member-admin.md), not a complete Wikidot administration suite.

## What it must do

- [ ] Route `/_admin` to `/-/admin` without querying a nonexistent wiki page.
- [ ] Keep the native administration permission check: an administrator can open the editor; anonymous users remain denied.
- [ ] Present a Site Manager heading, existing site information and a Site members link, without dumping loader/session data into a debug textarea.
- [ ] Opening and cancelling the site-info editor must not save changes.

## How it works

- [Replica status](../wiki/systems/cobalt-replica-status.md)

## Implementation inventory

- `framerail/src/routes/[x+5f]admin/+page.server.ts` — imported menu entry point.
- `framerail/src/routes/[x+2d]/admin/+page.svelte` — native administration screen.
- `framerail/src/lib/server/load/admin.ts` — existing backend permission handling and site-info action.

## Tests asserting this spec

- `framerail/tests/local/admin-pages.mjs` — real admin login, legacy entry navigation, editor open/cancel and anonymous denial, without submitting edits.

## Known gaps (current cycle)

- [ ] Independent local browser verification and public rollout remain pending.

## Out of scope

Adding a complete legacy Site Manager, bans, application moderation or changing membership permissions. The Applications wiki dashboard is a separate imported page, not an administration RPC.
