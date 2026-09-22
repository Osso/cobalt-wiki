# Authenticated browser transport

`tools/cobalt_migration/browser_transport.py` supplies the fetch callback for
[listing export](cobalt-listing-export.md), using an already authenticated browser.
It does not acquire credentials or start bulk acquisition.

## What it must do

- [x] Return `FetchResponse(status, html, retry_after)` through `make_browser_fetch(source_origin)`; preserve HTTP errors and Retry-After for the caller's retry policy.
- [x] Use the installed `browser-cli` executable with argv, captured text output, and bounded command timeouts; never print response HTML.
- [x] Start a same-origin credentialed GET with `redirect: manual`, then poll synchronous evaluations because browser-cli does not await promises. Detect `opaqueredirect` before reading a body and raise permanent `SourcePageRedirect`; never follow the redirect or acquire target identity. Listing export propagates this failure without advancing its checkpoint.
- [x] Guard every evaluation, including cleanup, against a changed origin; reject absolute URLs, authority-relative URLs, backslashes, fragments, and control/whitespace characters in requested paths.
- [x] Give each request a unique temporary slot; remove it and cancel outstanding work in `finally`. Report sole cleanup failures explicitly and annotate, rather than replace, an existing failure.
- [x] Decode direct JSON objects and JSON-encoded strings. Reject invalid protocol data without including captured bodies in error messages.
- [x] Raise `ConnectionError` for browser fetch rejection and `TimeoutError` for browser/command/deadline timeout. Keep origin, CLI, and protocol errors non-retryable.
- [x] Inject the command runner, monotonic clock, and sleep for tests without browser/network access.

## How it works

- [Transport lifecycle](../wiki/systems/cobalt-browser-transport.md) (documentation stub).

## Implementation inventory

- `tools/cobalt_migration/browser_transport.py`: callable browser transport and sanitized error handling.
- `tools/cobalt_migration/listing_export.py`: existing response type and caller-owned retry policy; unchanged.

## Tests asserting this spec

- `tests/cobalt_migration/test_browser_transport.py`: synthetic CLI failures and emitted JavaScript executed by local Node against a fake browser/fetch. Covers pending/done, opaque redirect without retries or listing checkpoint advancement, network rejection, browser timeout, polling deadline, origin changes, unique slots, cleanup precedence, and both JSON encodings. Node is a test-only dependency already present for this project; production transport uses Python standard library and registered browser-cli.

## Known gaps (current cycle)

- [x] Main accepted authenticated `/pagelist/p/277` through the registered browser transport: HTTP 200, 73 names, highest page 277. The protected listing run completed 277/277 pages with 6,092 unique fullnames.
- [x] Forward-only reconciliation maps the protected listing's 6,092 canonical names to all 6,092 archive source keys with zero gaps or collisions. This does not establish inverse name reconstruction, target import, ACL export, or deployment.

## Out of scope

- No CLI entry point, bulk run, login flow, cookie extraction, tab management, or source writes.
- No internal retries: listing export owns retries and Retry-After handling.
- Cleanup after origin change cannot touch the previous page's slot; the failure is explicit. The browser request has its own timeout. Cleanup receives a separate budget of at most five seconds after the fetch deadline.
