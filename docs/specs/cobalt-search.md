# Cobalt site search

Cobalt provides a server-side Meilisearch-backed `search:site` result page for current pages. Backend source is `deepwell/src/services/search/`; Framerail owns the header form and result route. Operational proof is tracked in the [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

- [x] Accept search only for the trusted request site and actor; ignore client-supplied site or user values.
- [x] Index title, slug, tags, and visible text extracted from the current compiled page HTML. Script, style, template, noscript, SVG, hidden, `aria-hidden`, display-none, `wj-hidden`, and `wj-invisible` content is excluded.
- [ ] Configure Meilisearch to match only `title`, `slug`, `tags`, and `body`; `site_id` is filterable only. The `cf1cdec` regression test is GREEN ongoing after exposing irrelevant results for a `site_id` query.
- [x] Filter Meilisearch candidates by site, then revalidate each current page and revision and its `Page` / `View` permission before returning it. Pagination offsets count visible results only; restricted text and totals do not escape.
- [x] Use a server-only Meilisearch URL and master key. The backend sends authenticated requests, waits for indexing tasks, and reports unavailable/index failures without returning the key.
- [ ] Enable search when either `MEILISEARCH_URL` or `MEILISEARCH_MASTER_KEY` is configured, require both values to validate, then supervise an indexing worker with the server. The worker must ensure the index, drain committed outbox rows without idle delay while work remains, poll after one idle second, and propagate a worker failure to server startup rather than die silently. Existing `SearchService` bounded retries remain unchanged.
- [x] Render a GET header form and a `search:site` result page with 20-result paging, escaped titles/snippets, empty-result and unavailable messages, a 200-byte UI query limit, and a maximum UI offset of 500.

## How it works

- [Replica proof boundaries](../wiki/systems/cobalt-replica-status.md)
- [Search system](../wiki/systems/cobalt-search.md) — operational/lifecycle design pending.

## Implementation inventory

- `deepwell/src/services/search/mod.rs` — Meilisearch client, explicit searchable/filterable index settings, visible-text extraction, candidate revalidation, freshness rejection, and paging.
- `deepwell/src/services/search/tests.rs` — fake-HTTP behavioral tests.
- `deepwell/src/services/page_revision/service.rs` — transactional search-outbox enqueue hooks for create, edit, delete, restore, and rerender at `c3ac4a1`; imports use these same page-revision services.
- `deepwell/src/services/search/worker.rs` — ensures the index, drains committed outbox batches, and polls idle for one second at `9a9a428`.
- `deepwell/src/start.rs` — validates configured search credentials and supervises the search worker with the server at `7bd7f21`.
- `deepwell/src/endpoints/search.rs`, `deepwell/src/api.rs`, `deepwell/src/endpoints/mod.rs` — `page_search` RPC registration.
- `framerail/src/lib/component/SearchBox.svelte`, `framerail/src/routes/+layout.svelte` — source-theme header form, mounted in the Wikidot header at `d07869d`.
- `framerail/src/lib/server/deepwell/search.ts` — trusted-context RPC client.
- `framerail/src/routes/search:site/+page.server.ts`, `+page.svelte` — query validation, result loading, and escaped rendering.
- `framerail/tests/search.test.ts` — server/component route tests.
- `deepwell/Cargo.toml` — `scraper` parses compiled HTML into visible search text; regex cannot reliably exclude hidden DOM subtrees.

## Tests asserting this spec

- `deepwell/src/services/search/tests.rs`: four fake-HTTP tests passed at `ff0844b` / `712a009`, covering permission-before-paging/no restricted totals, authenticated upsert/delete tasks, repeatable index configuration, and hidden compiled-HTML exclusion. `cf1cdec` adds a RED regression for explicit searchable/filterable settings after a `site_id` query returned 20 irrelevant hits; its GREEN result is still ongoing.
- Native lifecycle proof passed 2/2 at `c3ac4a1` (`/tmp/claude/cobalt-search-lifecycle-c3ac4a1-green.log`): create/edit/delete/restore/rerender changes commit their search-outbox work transactionally; import uses the same services.
- Real-database plus fake-Meilisearch freshness proof passed 1/1 at `183b083` (`/tmp/claude/page-search-freshness-183b083.log`): a same-revision rerender updates the committed outbox document and prevents stale stored body text from being returned. This is not real-index proof.
- `framerail/tests/search.test.ts`: five tests passed at `d63fd97`, covering header-form selectors/submission, trusted request headers and bounded RPC parameters, two pages of navigation, invalid offsets/empty query handling, and distinct empty/unavailable messages. `d07869d` mounts that form in the source header; this is source wiring, not browser acceptance.
- `76ba7bf` registers the backend RPCs consumed by the route; registration is not live-service proof.
- Local authenticated browser search passed 1/1 (`/tmp/claude/cobalt-search-browser-first.log`): expected fixture result, distinct 20+ result second page with Next/Previous navigation, plain snippets, no errors, and `401`/`noindex` protection.

## Known gaps (current cycle)

- [ ] `9a9a428` schedules the worker and `7bd7f21` supervises it when either search environment variable is present. A local build/startup and restart observed automatic index draining, last observed at 3,900 indexing; startup/supervision acceptance and completed backfill are not claimed.
- [ ] `cf1cdec` fixes default Meilisearch matching IDs by setting searchable fields to `title`/`slug`/`tags`/`body` and `site_id` filterable only; its regression GREEN result remains ongoing.
- [ ] Local proof has no production-deployment coverage. The 1/1 freshness result uses a real database with fake Meilisearch.

## Out of scope

Source ACL parity, deployment, result ranking tuning, and a browser-exposed Meilisearch credential are excluded. Search must not make source-private pages or content available.
