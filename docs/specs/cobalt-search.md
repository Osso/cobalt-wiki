# Cobalt site search

Cobalt provides a server-side Meilisearch-backed `search:site` result page for current pages. Backend source is `deepwell/src/services/search/`; Framerail owns the header form and result route. Operational proof is tracked in the [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

- [x] Accept search only for the trusted request site and actor; ignore client-supplied site or user values.
- [x] Index title, slug, tags, and visible text extracted from the current compiled page HTML. Script, style, template, noscript, SVG, hidden, `aria-hidden`, display-none, `wj-hidden`, and `wj-invisible` content is excluded.
- [x] Configure Meilisearch to match only `title`, `slug`, `tags`, and `body`; `site_id` is filterable only. The regression introduced at `cf1cdec` passed in the targeted 8/8 search suite after `b61806e` and `6c26c0d`.
- [x] Filter Meilisearch candidates by site, then revalidate each current page and revision and its `Page` / `View` permission before returning it. Pagination offsets count visible results only; restricted text and totals do not escape.
- [x] Use a server-only Meilisearch URL and master key. The backend sends authenticated requests, waits for indexing tasks, and reports unavailable/index failures without returning the key.
- [x] Enable search when either `MEILISEARCH_URL` or `MEILISEARCH_MASTER_KEY` is configured, require both values to validate, then supervise an indexing worker with the server. The worker ensures the index, drains committed outbox rows without idle delay while work remains, polls after one idle second, and propagates a worker failure to server startup rather than die silently. Existing `SearchService` bounded retries remain unchanged.
- [x] Render a GET header form and a `search:site` result page with 20-result paging, escaped titles/snippets, empty-result and unavailable messages, a 200-byte UI query limit, and a maximum UI offset of 500.

## How it works

- [Replica proof boundaries](../wiki/systems/cobalt-replica-status.md)
- [Search system](../wiki/systems/cobalt-search.md) — operational/lifecycle design pending.

## Implementation inventory

- `deepwell/src/services/search/mod.rs` — Meilisearch client, explicit searchable/filterable index settings, visible-text extraction, candidate revalidation, freshness rejection, and paging.
- `deepwell/src/services/search/tests.rs` — fake-HTTP behavioral tests.
- `deepwell/tests/page_search_permissions.rs` — isolated real-database/fake-Meilisearch authorization regression for `page_search`.
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

- `deepwell/src/services/search/tests.rs`: four fake-HTTP tests passed at `ff0844b` / `712a009`, covering permission-before-paging/no restricted totals, authenticated upsert/delete tasks, repeatable index configuration, and hidden compiled-HTML exclusion. The `cf1cdec` RED regression for explicit searchable/filterable settings passed in the targeted 8/8 search suite after `b61806e` and `6c26c0d`.
- `deepwell/tests/page_search_permissions.rs`: current exact-revision proof passed 1/1 at `b3784fc758faf41ce875276fa914c26f018ebae6` with `cargo test --test page_search_permissions` (test 2.60s; compile 2m32s; `/tmp/claude/cobalt-search-current-acl.log`; ledger `/tmp/claude/cobalt-search-current-acl-proof.json`). The isolated `cobalt_test`/Redis DB 5 fixture uses fake Meilisearch and proves denied same-site, foreign-site, and forged-site candidates are excluded; denied titles/snippets do not leak; visible-result pagination remains correct; and a request `page_reference` cannot grant an unprivileged actor access. Earlier historical passing evidence without an exact revision is retained, but this is now current proof. Independent verifier 639 audited revision `b3784fc758faf41ce875276fa914c26f018ebae6`, the ledger/log, the 326-line test, and trusted-context/permission-output source without rerunning it. This does not prove all indexed-body or source-ACL parity.
- Native lifecycle proof passed 2/2 at `c3ac4a1` (`/tmp/claude/cobalt-search-lifecycle-c3ac4a1-green.log`): create/edit/delete/restore/rerender changes commit their search-outbox work transactionally; import uses the same services.
- Real-database plus fake-Meilisearch freshness proof passed 1/1 at `183b083` (`/tmp/claude/page-search-freshness-183b083.log`): a same-revision rerender changes the compiled body and prevents stale indexed body text from being returned. This is not real-index proof.
- `framerail/tests/search.test.ts`: five tests passed at `d63fd97`, covering header-form selectors/submission, trusted request headers and bounded RPC parameters, two pages of navigation, invalid offsets/empty query handling, and distinct empty/unavailable messages. `d07869d` mounts that form in the source header; this is source wiring, not browser acceptance.
- `76ba7bf` registers the backend RPCs consumed by the route; registration is not live-service proof.
- Independent verifier 330 passed formatting, check, authenticated browser search 1/1, and Framerail search tests 5/5. Runtime 3989 observed `docs=3989`, `live pending=0`, an idle index, one matching search fixture result, and `401`/`noindex` protection. `b61806e` and `6c26c0d` fixed the remaining search style/readability findings; targeted search tests passed 8/8. The final bounded evidence audit confirmed 3,991 indexed documents matching 3,991 live local pages, zero pending updates, and an idle index (`/tmp/claude/cobalt-final-local-inventory.json`). At `3668a7a8e`, loopback hydrated browser search passed 1/1 with prepared results and distinct pagination (`/tmp/claude/cobalt-search-3668a7a8e-browser.log`); verifier 624 then passed ESLint, Prettier, Svelte check (0 errors/0 warnings), and readability (max cognitive 3/cyclomatic 8; no findings) (`/tmp/claude/cobalt-search-3668a7a8e-{eslint,prettier,svelte-check,readability-metrics}.log`).

## Current revision-ID evidence

- Read-only main comparison observed 6,114 live pages and 6,114 indexed site pages on 2026-09-24, over seven pagination batches (six of 1,000 and one of 114), with no broken current-revision pointers or revision-ID mismatches (`/tmp/claude/cobalt-search-revision-parity.json`). This compares current revision IDs only; it is not body-byte or ACL/authorization proof.

## Current content equality evidence

- Protected read-only audit on 2026-09-25 compared 6,114 database pages with 6,114 indexed documents across seven batches (six of 1,000 and one of 114). `metadata-summary.json` records zero mismatches, missing values, or extra values for each of its five checked metadata fields. `body-summary.json` and `body-results.jsonl` record exact body equality after recomputing every page body with the current Rust `plain_body` function copied to `/tmp/claude/cobalt-search-body-audit.rs`; that copy was confirmed identical to `mod.rs`.
- `snapshot-stability.json` records identical database `compiledHTML` and source-metadata snapshots before and after the audit; `pending.log` records zero pending work. `index.json` confirms `pages` indexes `page_id`, configures `site_id` as filterable, and limits searchable fields to `title`, `slug`, `tags`, and `body`.
- This closes the local snapshot body-equality gap. It does not establish lifecycle transition timing, ACL/authorization behavior, source-search parity, or production behavior; fake-Meilisearch lifecycle proof remains separate. Independent audit gate remains pending. The audit retains the source-search-disabled and 43 unreconstructable fingerprint/history boundaries.

## Known gaps (current cycle)

- The SSR cache-isolation batch retained 22 passing tests; `20a3afc` corrected the three incomplete doctype fixtures, with their targeted group passing 4/4. Final scoped frontend checks and the bounded independent evidence audit passed.
- [x] Production search enabled 2026-09-24; see [replica status](../wiki/systems/cobalt-replica-status.md#production-meilisearch-search).
- [ ] `MEILISEARCH_MASTER_KEY` carries a scoped key in production; the variable name predates that.
- Wikidot's own search is disabled on the source site ("Search is temporarily unavailable"), so result-page markup has no live source to match.

## Out of scope

Source ACL parity, result ranking tuning, and a browser-exposed Meilisearch credential are excluded. Search must not make source-private pages or content available.
