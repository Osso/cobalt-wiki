# Cobalt public page-pane reads

Framerail's legacy page-pane actions in `src/lib/server/load/page.ts` expose metadata and history for their routed page. Security evidence belongs in the [audit](../wiki/systems/cobalt-security-audit.md).

## What it must do

- [x] File listing, native history/revision, parent list, vote list, score and lock history require the same current Page/View access as the routed page: denied pages return 403, missing pages 404, without returning private read data.
- [x] Submitted page IDs must match the authorized route; submitted site IDs, where used, must match trusted ingress context. Authorized pane reads remain available.
- [x] Root-route reads resolve the configured default page, preserving normal loader routing semantics.
- [x] Routed read authorization supplies the request languages and configured default locale, including requests without Accept-Language or with a wildcard language range. Wildcards are negotiation ranges, not locale identifiers. Private-page denial must not become a translation error carrying page diagnostics; native/imported history and backlinks use the same routed check.
- [x] Vote cast and cancel use the same routed-page Page/View check: another page or an unviewable page returns 403 without changing votes. `vote_set` accepts only -1, 0 or +1 (Wikidot up/down sum); other values fail `BadRequest` without storing a vote. Tests: `framerail/tests/actor-session.test.ts`, `deepwell/tests/page_vote.rs`.

## Player-page privacy

- Player-category pages are private to site members (including moderator, administrator and root roles); anonymous visitors and signed-in nonmembers must not receive page content, source or form data.
- Provision these category-scoped Page/View grants through `tools/cobalt_migration/private_categories.sql`; changing grants requires invalidating the site's permission cache. Default public-page access remains unchanged.

## Deleted-page restore lookup

- [x] The public `deletedGet` action remains available to restore UI and binds submitted site/slug to trusted request context, including the configured root default page.
- [x] `page_get_deleted` requires an authenticated, unrestricted request actor and matching request site/slug. Every candidate deleted record must pass the existing restore Page/Edit authorization using its actual category and current locks before metadata is returned; denials return 3106 without metadata.
- [x] Authorized editors retain deleted-record metadata lookup without new Page/View or administrator requirements. Hidden-field policy and trusted direct getters are unchanged.
- [x] Each allowed or denied deleted-record lookup preserves every full Page and PageRevision model in the fixture site. Session renewal and unrelated database rows are outside that preservation proof.
- Proof: `deepwell/src/api/deleted_page_access_tests.rs` exercises registered HTTP JSONRPC with PostgreSQL; `framerail/tests/deleted-page-access.test.ts` exercises registered SSR actions with RPC fixtures.

## How it works

- [Security audit](../wiki/systems/cobalt-security-audit.md)

## Implementation inventory

- `framerail/src/lib/server/load/page-read.ts` — shared routed-page authorization.
- `framerail/src/lib/server/load/page.ts` — seven protected public read actions.

## Tests asserting this spec

- `framerail/tests/page-read-authorization.test.ts` — actual Vite SSR actions with RPC fixtures; 48 targeted cases cover denied/missing/authorized pages, foreign target IDs, configured root pages and route extras. Lock history has no site selector and uses trusted request context.

## Known gaps (current cycle)

- [ ] Evidence is SSR-action/RPC-fixture proof, not real database or deployed private-page acceptance.
- [ ] Per-record hidden fields have separate contracts; active-page authorization does not establish additional field-redaction policies.

## Out of scope

Changing trusted loopback getter RPC contracts or inventing new confidentiality rules for visible-page voter identities and parent names.
