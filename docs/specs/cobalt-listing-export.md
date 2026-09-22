# Cobalt listing export

## Contract

`export_listing(source_origin, checkpoint_path, fetch, *, sleep, interval_seconds=1.0, jitter, now)` acquires `/pagelist/p/N` sequentially through an injected GET callback. The callback returns `FetchResponse(status, html, retry_after=None)`; it raises `ConnectionError` or `TimeoutError` for retryable transport failures. Other exceptions propagate without retry.

- Start at page 1; continue through the highest page discovered by `parse_page_listing`. Deduplicate canonical fullnames in first-seen order, retaining underscores literally. Repeated clamped listing blocks do not duplicate names.
- Persist only schema version 1, exact source origin, completed page number, highest page number, and accumulated fullnames. No HTML or session values are stored. Resume rejects invalid schema, origin, pagination, or names before issuing GETs; a completed checkpoint performs no GETs.
- Checkpoint parent must be owner-owned and inaccessible to group/others; create a missing immediate parent with mode 0700. Existing checkpoint must be an owner-owned regular 0600 file, not a symlink. Publish each successful page through a 0600 temporary file, flush/fsync, then atomic replacement. Failure leaves the previous checkpoint intact.
- A failed HTTP response or parser error never advances the checkpoint. Retry only HTTP 429, HTTP 5xx, and the documented transport exceptions, at most four attempts per page. Exponential delays include injected jitter; valid Retry-After seconds or HTTP dates provide a minimum delay. Invalid Retry-After is an explicit error. Permanent statuses and parser errors are not retried.
- Wait at least `interval_seconds` between actual GET calls within an invocation, including retries. Default is one second. The first GET after invocation/resume has no delay; rate-limit timestamps are not persisted.

## Proof boundary

Targeted synthetic tests exercise three pages with repeats and an expanding page count, completed/interrupted resume, permanent failure, retry exhaustion, Retry-After seconds/date, transport failure, parser failure, file permissions, origin/schema validation, and interrupted atomic replacement. These are development tests, not independent acceptance.

Main accepted authenticated `/pagelist/p/277` through the browser transport: HTTP 200, 73 names, highest page 277. A protected complete run recorded 277/277 pages and 6,092 unique literal fullnames in `listing-checkpoint.json`. Forward-only reconciliation maps every canonical fullname with `:` replaced by `_` to all 6,092 archive source keys, with zero gaps or collisions. That evidence does not allow inverse archive-name reconstruction. No target import or deployment is provided.
