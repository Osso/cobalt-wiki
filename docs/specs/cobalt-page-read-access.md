# Cobalt public page-pane reads

Framerail's legacy page-pane actions in `src/lib/server/load/page.ts` expose metadata and history for their routed page. Security evidence belongs in the [audit](../wiki/systems/cobalt-security-audit.md).

## What it must do

- [x] File listing, native history/revision, parent list, vote list, score and lock history require the same current Page/View access as the routed page: denied pages return 403, missing pages 404, without returning private read data.
- [x] Submitted page IDs must match the authorized route; submitted site IDs, where used, must match trusted ingress context. Authorized pane reads remain available.
- [ ] Root-route reads resolve the configured default page, preserving normal loader routing semantics.

## How it works

- [Security audit](../wiki/systems/cobalt-security-audit.md)

## Implementation inventory

- `framerail/src/lib/server/load/page-read.ts` — shared routed-page authorization.
- `framerail/src/lib/server/load/page.ts` — seven protected public read actions.

## Tests asserting this spec

- `framerail/tests/page-read-authorization.test.ts` — actual Vite SSR actions with RPC fixtures; 34 targeted cases cover denied/missing/authorized pages and foreign target IDs. Lock history has no site selector and uses trusted request context.

## Known gaps (current cycle)

- [ ] Evidence is SSR-action/RPC-fixture proof, not real database or deployed private-page acceptance.
- [ ] Deleted-page lookup and per-record hidden fields have separate lifecycle contracts; active-page authorization does not settle those policies.

## Out of scope

Changing trusted loopback getter RPC contracts or inventing new confidentiality rules for visible-page voter identities and parent names.
