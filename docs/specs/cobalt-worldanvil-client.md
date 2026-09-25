# World Anvil create-only transport

`tools/cobalt_migration/worldanvil_client.py` exposes `WorldAnvilClient` with only `list_articles(world_id)`, `get_article(article_id)`, `get_world(world_id)`, and `create_article(world_id, article)`. Import selection, conversion, checkpoints, and live writes belong to the importer, not this transport.

## Contract

- Use Boromir at `https://www.worldanvil.com/api/external/boromir`. Constructor accepts application key, auth token, User-Agent and positive timeout; send both credential headers and JSON Content-Type on every request. A loopback HTTP base URL is supported for local integration tests; no redirect follows.
- `list_articles` POSTs `/world/articles?id=WORLD` with `{"offset": 0, "limit": 50}`, reading each successful `{"success": true, "entities": [...]}` envelope and increasing offset by returned entity count until a page contains fewer than 50 entries. Exactly 50 entries require one further terminal request. Return ordered article refs. Reject malformed or failed envelopes, oversized or non-array `entities`, missing/invalid UUID or title, and duplicate IDs across or within pages.
- `get_article` GETs `/article?id=UUID&granularity=2`; `get_world` GETs `/world?id=UUID&granularity=2`. Require JSON objects, valid UUID and nonblank title, requested identity, and no explicit `success: false`.
- `create_article` PUTs `/article` **without an ID parameter**. It requires a nonblank title and templateType, forbids caller-supplied `id` and `world`, and adds `world: {"id": WORLD}`. Return validated created article reference. Never update or delete existing articles; no PATCH/DELETE method is exposed.
- Retry only reads (including the read-only listing POST) on transient HTTP 408/429/5xx (500, 502, 503, 504) and connection errors, at most four attempts total with exponential delay, jitter, and Retry-After (maximum 30 seconds, otherwise fail). No automatic retry for PUT, including timeout or uncertain response: a second PUT could create another UUID. Permanent errors and malformed responses fail immediately.
- Error messages reveal method, fixed API path and status/category only. HTTP failures retain a separate `response_body` for validation diagnostics with both credential values redacted; callers must treat provider text as potentially private and avoid printing it indiscriminately. No query values or provider body are added to the exception message.

## Proof and boundaries

`tests/cobalt_migration/test_worldanvil_client.py` exercises a local HTTP server: 75 article refs across two pages, exact-50 terminal pagination, the observed `{"success": true, "entities": [...]}` listing envelope, GET granularity and credential headers, create-only effect preserving an existing article, failed PUT attempted once, read retries and Retry-After, malformed responses and duplicate IDs. No production World Anvil write has occurred as of this audit. Importer/conversion behavior and source acquisition are not covered here.
