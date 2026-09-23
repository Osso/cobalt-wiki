# Cobalt site search

Cobalt provides a server-side Meilisearch-backed `search:site` result page for current pages. Backend source is `deepwell/src/services/search/`; Framerail owns the header form and result route. Operational proof is tracked in the [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

- [x] Accept search only for the trusted request site and actor; ignore client-supplied site or user values.
- [x] Index title, slug, tags, and visible text extracted from the current compiled page HTML. Script, style, template, noscript, SVG, hidden, `aria-hidden`, display-none, `wj-hidden`, and `wj-invisible` content is excluded.
- [x] Filter Meilisearch candidates by site, then revalidate each current page and revision and its `Page` / `View` permission before returning it. Pagination offsets count visible results only; restricted text and totals do not escape.
- [x] Use a server-only Meilisearch URL and master key. The backend sends authenticated requests, waits for indexing tasks, and reports unavailable/index failures without returning the key.
- [x] Render a GET header form and a `search:site` result page with 20-result paging, escaped titles/snippets, empty-result and unavailable messages, a 200-byte UI query limit, and a maximum UI offset of 500.

## How it works

- [Replica proof boundaries](../wiki/systems/cobalt-replica-status.md)
- [Search system](../wiki/systems/cobalt-search.md) — operational/lifecycle design pending.

## Implementation inventory

- `deepwell/src/services/search/mod.rs` — Meilisearch client, visible-text extraction, candidate revalidation, and paging.
- `deepwell/src/services/search/tests.rs` — fake-HTTP behavioral tests.
- `deepwell/src/endpoints/search.rs`, `deepwell/src/api.rs`, `deepwell/src/endpoints/mod.rs` — `page_search` RPC registration.
- `framerail/src/lib/component/SearchBox.svelte` — source-theme header form.
- `framerail/src/lib/server/deepwell/search.ts` — trusted-context RPC client.
- `framerail/src/routes/search:site/+page.server.ts`, `+page.svelte` — query validation, result loading, and escaped rendering.
- `framerail/tests/search.test.ts` — server/component route tests.
- `deepwell/Cargo.toml` — `scraper` parses compiled HTML into visible search text; regex cannot reliably exclude hidden DOM subtrees.

## Tests asserting this spec

- `deepwell/src/services/search/tests.rs`: four fake-HTTP tests passed at `ff0844b` / `712a009`, covering permission-before-paging/no restricted totals, authenticated upsert/delete tasks, repeatable index configuration, and hidden compiled-HTML exclusion.
- `framerail/tests/search.test.ts`: five tests passed at `d63fd97`, covering header-form selectors/submission, trusted request headers and bounded RPC parameters, two pages of navigation, invalid offsets/empty query handling, and distinct empty/unavailable messages.
- `76ba7bf` registers the backend RPCs consumed by the route; registration is not live-service proof.

## Known gaps (current cycle)

- [ ] Index lifecycle, transactional outbox, worker, and backfill are pending; current committed code does not prove page changes reach Meilisearch.
- [ ] Local browser acceptance for the header form and result route is pending.
- [ ] Local Meilisearch proof covers no real indexed page, database integration, restart behavior, or production deployment.

## Out of scope

Source ACL parity, deployment, result ranking tuning, and a browser-exposed Meilisearch credential are excluded. Search must not make source-private pages or content available.
