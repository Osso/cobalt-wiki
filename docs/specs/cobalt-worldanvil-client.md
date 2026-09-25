# World Anvil create-only transport

`tools/cobalt_migration/worldanvil_client.py` exposes `WorldAnvilClient` with only `list_articles(world_id)`, `get_article(article_id)`, `get_world(world_id)`, and `create_article(world_id, article)`. Import selection, conversion, checkpoints, and privacy decisions belong to the caller.

## Contract

- Use Boromir at `https://www.worldanvil.com/api/external/boromir`. Constructor accepts application key, auth token, User-Agent, and positive timeout; send both credential headers and JSON Content-Type. Only a loopback HTTP base URL is accepted for local integration tests; redirects are not followed.
- `list_articles` POSTs `/world/articles?id=WORLD` with `{"offset": 0, "limit": 50}`. It reads successful `{"success": true, "entities": [...]}` pages until a page has fewer than 50 entities; exactly 50 requires one terminal request. It rejects malformed envelopes, oversized/non-array entities, invalid IDs/titles, and duplicate IDs.
- `get_article` and `get_world` GET their respective endpoint with `id` and `granularity=2`; require a valid requested identity, nonblank title, JSON object, and no explicit `success: false`.
- `create_article` PUTs `/article` without an ID parameter. It requires nonblank title/template type, forbids caller `id` and `world`, adds `world: {"id": WORLD}`, and validates the created reference. No update/delete method exists.
- Retry reads only, including listing POSTs, for transient 408/429/500/502/503/504 and connection failures: at most four attempts with exponential delay, jitter, and `Retry-After` capped at 30 seconds. Never retry PUT, including uncertain responses.
- Failure messages contain only method, fixed API path, and HTTP status/category. For HTTP failures, `WorldAnvilError.response_body` retains provider text after redacting both configured credential values. It is not placed in the exception message.

## Caller privacy guard

`response_body` may contain private provider diagnostics. Callers must not print or otherwise expose it by default; inspect it only through a deliberate privacy-safe diagnostic path. The transport performs credential redaction, not general provider-data sanitization.

## Tests asserting this spec

`tests/cobalt_migration/test_worldanvil_client.py` uses a local HTTP server for pagination (including exact-50 terminal page), credential headers, GET granularity, create-only behavior, failed PUT once, retries/Retry-After, malformed responses, duplicate IDs, and retained redacted HTTP response bodies.

## Out of scope

- Source acquisition/conversion, inventory selection, journals, binary uploads, browser interaction, and all live migration policy.
