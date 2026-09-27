# Page history listing

The `page_history_list` read RPC lists preserved Wikidot history or native local history for one authorized page. Its backend lives in `deepwell/src/services/import/history_listing.rs`; source acquisition and storage remain governed by [history import](cobalt-history-import.md).

## What it must do

- [x] Accept `{site_id,page_id,origin:'wikidot'|'local',page?,per_page?,filters?}`; default to page 1, 20 rows, all flags, and require positive pages and per-page values 10, 20, 50, 100, or 200.
- [x] Return `{origin,page,per_page,total,total_pages,available:{wikidot:boolean,local:boolean},rows}` with database-counted totals and bounded, newest-first SQL pages for the selected origin. `available` reports datasets, not counts; do not merge native and source revision numbers.
- [x] Each row identifies stored revision ID/number, actual flags, optional stored author name and ID, optional local `author_slug` (native authors, and imported authors whose Wikidot user ID is an activated local account), RFC3339 timestamp, comments, current marker, and nullable representation. Unknown authors remain unnamed; only a native revision matching the page's latest native revision is current.
- [x] Match the seven supported filters with OR semantics before counting and paging; `all:true` or no selections means all. Tags (`A`) and metadata (`M`) are separate: imported revisions use stored flags, native revisions use `tags` and `alt_title` changes respectively. Native F cannot match absent native file evidence.
- [x] Reject cross-site pages and denied Page View reads before exposing counts or rows; suppress hidden native comments.
- [x] Preserve imported cursor reads, source reads, current page, stored revisions, and imported display-decoded representation without rewriting data.

## How it works

- [History import](cobalt-history-import.md) documents the distinct preserved source representation.

## Implementation inventory

- `deepwell/src/services/import/history_listing.rs` — counts and pages both origins and maps their rows.
- `deepwell/src/services/import/history_listing_structs.rs` — request and response contract.
- `deepwell/src/services/import/history.rs` — shared Page View authorization.
- `deepwell/src/endpoints/import.rs` — RPC entry point (registered by integration owner).

## Tests asserting this spec

- `deepwell/tests/page_history_listing.rs` — real database pages, flags, identity, preservation, invalid input, and denied reads; native/imported reads 4/4 and listings 4/4.

## Known gaps (current cycle)

- [ ] Comprehensive local-browser proof for History controls remains pending. Root `/` resolves `home:_public` (57) through the History action's `route:null`; named `home:start` (240) is the other browser fixture. Neither establishes complete browser behavior or visual parity.
- [ ] Independent type, lint, and readability corrections plus screenshot-spacing updates remain pending before the current gate can close.

## Out of scope

- Source fetches, author profile fetches, history mutation, invented native file flags, byte-exact source claims, and production deployment.
