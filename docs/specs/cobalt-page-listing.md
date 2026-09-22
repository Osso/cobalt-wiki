# Cobalt page listing

`tools/cobalt_migration/page_listing.py` extracts canonical fullnames and pagination from supplied Wikidot listing HTML using Python's standard library. It performs no I/O and does not execute scripts. See [page metadata](cobalt-page-metadata.md) for subsequent per-page parsing.

## What it must do

- [x] `parse_page_listing(html, source_origin)` returns `{"fullnames": [...], "highest_page": N}`. Names preserve document order and deduplicate across all `#page-content .list-pages-box` blocks in one response. An empty block is valid; absent pagination means page 1.
- [x] Read the highest observed number from listing `.pager-no` text and same-origin `.pager a` paths `/pagelist/p/N`. Repeated/clamped blocks do not introduce duplicate names within a response; repeated names across separate responses remain caller-owned.
- [x] Accept relative, root-relative, protocol-relative, and absolute HTTP(S) links only at the supplied origin, comparing scheme, hostname and effective port. Exclude pager links, `Edit` text (case-insensitive with surrounding whitespace ignored), fragment-only/query-only links, off-origin links, multi-segment routes and inert script/style/template content.
- [x] Preserve hidden names, underscores, case and Unicode. Decode URL percent escapes exactly once with strict UTF-8; reject malformed escapes, invalid UTF-8, encoded path separators, whitespace and control characters instead of fabricating identities. Encoded percent signs remain literal after one decode. Path-bearing fragment links identify the same page.
- [x] Raise `PageListingError` for missing listing structure or invalid source origins. Reuse `SourcePageUnavailable` for recognized denial/not-found titles or content prefixes, independently of HTTP status.

## How it works

- [Listing architecture](../wiki/systems/cobalt-page-listing.md) (documentation stub).

## Implementation inventory

- `tools/cobalt_migration/page_listing.py` — pure HTML/URL parser and explicit listing errors.
- `tools/cobalt_migration/page_metadata.py` — existing shared source-unavailable exception; unchanged.

## Tests asserting this spec

`tests/cobalt_migration/test_page_listing.py`: nine synthetic behavioral tests, including two listing responses with repeated unpaginated blocks and different paginated entries. Targeted RED/GREEN passed; no authenticated response was used. This is synthetic-only development proof, not live acceptance.

## Known gaps (current cycle)

- [ ] Independently validate against authenticated listing responses, including actual pagination markup.

## Out of scope

- Crawling, cross-response deduplication, checkpoints and browser transport: acquisition integration owns these.
- Archive-key inference, metadata extraction, storage and private fixtures: excluded from this parser slice.
- Treating observed pagination as proof of complete export: page counts and source state require acquisition reconciliation.
