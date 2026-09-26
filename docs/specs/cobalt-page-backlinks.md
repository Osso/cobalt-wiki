# Page backlinks

Deepwell's `page_backlinks` read API returns incoming page links and inclusions visible to the current viewer. The source is `deepwell/src/services/link/backlinks.rs`; this API does not change the existing raw `page_get_links_to` endpoint.

## What it must do

- [x] Accept `{site_id, page_id}` and return separate `links` and `inclusions` arrays. Each entry contains only `{page_id, slug, title}` from the current source page and its latest revision.
- [x] Resolve the requested target as a live page in the requested site and deny viewers without its Page/View permission before returning any connections.
- [x] Include only live, same-site source pages the current viewer can view with Page/View; omit private, deleted, and foreign sources without revealing their IDs or counts. Banned viewers cannot view public targets.
- [x] Group `Link` connections as links and `IncludeMessy`/`IncludeElements` connections as inclusions; exclude redirects and components.
- [x] Deduplicate within each group and order entries by slug, independently of connection count or insertion order.
- [x] Repeated reads do not change page revisions.

## How it works

- [Page connections and permissions](../relations.md)

## Implementation inventory

- `deepwell/src/services/link/backlinks.rs` — permission-filtered read and minimal response types.
- `deepwell/src/services/link/mod.rs` — service exports.
- `deepwell/src/endpoints/link.rs` — request parsing and endpoint.
- `deepwell/src/api.rs` — RPC registration.

## Tests asserting this spec

- `deepwell/tests/page_backlinks.rs` — isolated transactional site, source, permission, grouping, denial, and read-only cases.

## Known gaps (current cycle)

- [ ] No consumer of `page_backlinks` in the page UI within this backend slice.

## Out of scope

- The existing unfiltered `page_get_links_to` response is unchanged; replacing or restricting it is separate work.
- No page rendering, print UI, or mutation endpoint is included.
