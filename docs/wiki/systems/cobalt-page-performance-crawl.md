# Production page-performance crawl

verified: 2026-09-29 UTC

## Result

All **6,110 live-page URLs** in the starting database inventory were attempted. **6,071 eventually returned HTTP 200; 39 returned 403** to the anonymous crawler. HTML responses were generally responsive, but image delivery was not consistently healthy under browser request bursts. This is not a claim that every page loads correctly or quickly for every user.

The HTML pass ran from **04:55:30 to 05:51:50 UTC**, followed by targeted repeat and browser measurements. No optimization or service-control changes were made by this measurement workflow.

## HTML timings

Percentiles below describe final successful responses, not failed attempts or complete browser page loads. They are URL-weighted, not traffic-weighted. TTFB includes a fresh connection's DNS/TLS/network time; it is not pure backend execution time.

| Metric | All successful HTML responses | Selected warm rechecks |
|---|---:|---:|
| Observations | 6,071 | 90 requests across 30 pages |
| Median TTFB | 357 ms | 125 ms |
| 95th-percentile TTFB | 502 ms | 255 ms |
| 99th-percentile TTFB | 639 ms | — |
| Maximum successful TTFB | 1,360 ms | 553 ms |

Only seven successful first-pass responses exceeded one second to first byte. The selected repeat cohort includes all seven, recovered failures, large responses and category/control samples; it is not a random estimate of site-wide warm latency.

### Outliers and failures

- `writing:2024-07-07-disgusting-eels-justifiably-slain-white-s` had the highest successful TTFB, 1.36 s. About 491 ms was connection/TLS setup; do not attribute the entire delay to rendering.
- `writing:2022-12-06-after-tempest` had the longest successful **HTML transfer**, 5.26 s despite 613 ms TTFB and only 21.7 KB transferred. Three later HTML transfers took 303–624 ms; a subsequent browser load took 1.33 s. The original transfer stall's cause was not isolated.
- `writing:2026-04-09-vale-campaign:mistfall-s-on-fire` initially timed out after 20 seconds without receiving bytes. Its retry succeeded in 181 ms. Cause remains unattributed.
- `writing:2025-06-08-i-suppose-we-did-just-kill-deathwing` initially returned 502, then succeeded on retry. The initial failure coincided with a clean Framerail restart at 05:30:44 UTC. The harness did not request that restart; its cause was not determined.
- The 39 denied URLs comprised 31 application, four admin, two navigation and two members-only pages. Their authenticated content performance was not measured or bypassed.

## Browser sample

**28 explicitly selected public URLs** were loaded sequentially in fresh anonymous Chromium contexts at 1440×1000. Selection covered the named homepage/rosters, slow first responses, largest HTML responses, one successful page from each public category, recovered failures and the HTML-transfer outlier. These are selected samples, not all 6,110 pages.

All 28 document navigations returned 200 with no uncaught JavaScript errors or navigation timeouts. Median document `load` was **551 ms**, maximum **2.20 s**. These load timings include failed resource responses: a quick `load` event does not mean every image succeeded.

| Page | Load event | LCP | Recorded transfer | Failed resource responses |
|---|---:|---:|---:|---:|
| `roster` | 2.20 s | 1.48 s | 9.00 MB | 5: two 404s and three 5xx |
| `roster-by-race` | 1.98 s | 1.66 s | 7.36 MB | 2 × 404 |
| `roster-by-class` | 1.59 s | 1.53 s | 9.25 MB | 2 × 404 |
| `pet-icons` | 1.45 s | 0.62 s | 3.38 MB | 53 × 502 |
| `home:start` | 1.03 s | 0.58 s | 1.73 MB | 0 |

### Remaining image-delivery problem

Across the browser samples there were **55 image 502 responses and one image 500 response**. All **56 URLs returned 200 when rechecked individually at low rate**. Nginx logged **23** `256 worker_connections are not enough` alerts during the browser window, specifically around roster/class loading. This establishes a real capacity limit and burst-sensitive media failures; it does not prove that this one limit caused every failed asset, particularly the later pet-icon burst.

There were also **29 resource 404s** and **six 400s**. The 404s include missing roster icons and writing-page requests whose apparent filename starts with `style="margin:`. The 400s were on direct template viewing. These are separate resource/content-compatibility findings; no fixes were applied.

The remaining priority suggested by this run is **image-request fan-out and delivery capacity/revalidation**, not a broad HTML-renderer or database rewrite. See the earlier [roster profile](cobalt-roster-performance.md) for the measured image-serving paths and options. Optimization still requires user selection.

## Coverage, method and limits

- Inventory: active site `6000000` page rows, captured before the run and reconciled afterward. Both snapshots contained 6,110 identical page IDs; no latest revision IDs changed. This is not a full-database immutability audit.
- HTML crawler: same-origin canonical GETs only, one request at a time, at most two starts/second, 20-second timeout, explicit English locale, no authentication, no redirect following, and no asset fetching. Transient attempts were retained separately; 6,112 attempts produced 6,110 final outcomes. Rate-limit, challenge, certificate and consecutive-failure circuit breakers did not pause the crawl.
- The first pass was **not uniformly cold**: 230 inventory rows already had the current renderer identity. Afterward, 6,070 rows had it, versus 230 before. Ordinary reads can warm stale compiled HTML. No forced invalidation or rerender sweep was issued. `search:site` is a successful native search URL whose stored wiki revision retained its older identity.
- Repeat measurements: three requests each across 30 selected pages, all 200. The additional transfer-outlier records store completion timestamps (`finished_utc`), not request-start timestamps.
- Browser contexts were deliberately cache-cold. Read-only request interception blocks non-read methods/external main-frame navigation and disables the HTTP cache. No requests were blocked and no safety budget was reached. Each sample had a 30-second navigation deadline, 500-request and 50 MiB budgets, plus a 1.2-second observation window after `load`. Embedded assets were fetched; no forms were submitted.
- Browser resource totals come from recorded completed CDP responses; redirect hops are not separately counted. Browser results are from one machine/network, without simulated mobile CPU/network conditions. They do not measure logged-in/admin-specific work or private content.
- Backend artifact remained `b2201f800f5a0c51219dbfff137917fc10bd9f44b8dac1f9dbf965292a6febdb`. End-of-run Framerail files matched the previously deployed build; the restart above is retained as a measurement condition. Deepwell, Framerail and WWS were active after sampling.

## Evidence

Protected raw data and per-page CSV:

`/home/osso/.local/share/cobalt-wiki/performance/20260929T045234Z/`

- `inventory.csv`, `inventory-after.csv`, `inventory-reconciliation.json`, `run.json` — scope and deployment context.
- `measurements.jsonl`, `html-pages.csv`, `summary.json` — all attempts, final per-page outcomes and aggregates.
- `warm/measurements.jsonl`, `warm-extra.json` — 90 repeat observations.
- `browser-selection.json`, `browser-extra-selection.json`, `browser-results.jsonl`, `browser-extra-results.jsonl`, `browser-summary.json` — 28-page selection and browser measurements.
- `asset-failures.json`, `asset-rechecks.json`, `browser-gateway-journal.txt`, `html-transient-restart-journal.txt`, `final-service-health.txt`, `frontend-artifact-comparison.txt` — resource failures, bounded rechecks and runtime context.

Early methodology audit: `/tmp/cobalt-crawl-method-audit.md`. Final independent audit is recorded alongside the run artifacts.
