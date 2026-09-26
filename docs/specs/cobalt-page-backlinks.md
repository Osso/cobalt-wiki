# Page backlinks

Deepwell's `page_backlinks` read API returns incoming page links and inclusions visible to the current viewer. The source is `deepwell/src/services/link/backlinks.rs`; this API does not change the existing raw `page_get_links_to` endpoint.

## What it must do

- [x] Accept `{site_id, page_id}` and return separate `links` and `inclusions` arrays. Each entry contains only `{page_id, slug, title}` from the current source page and its latest revision.
- [x] Resolve the requested target as a live page in the requested site and deny viewers without its Page/View permission before returning any connections.
- [x] Include only live, same-site source pages the current viewer can view with Page/View; omit private, deleted, and foreign sources without revealing their IDs or counts. Banned viewers cannot view public targets.
- [x] Group `Link` connections as links and `IncludeMessy`/`IncludeElements` connections as inclusions; exclude redirects and components.
- [x] Deduplicate within each group and order entries by slug, independently of connection count or insertion order.
- [x] Repeated reads do not change page revisions.
- [x] Render the visible Backlinks control with separately filtered links and inclusions, including an empty inclusions result.

## How it works

- [Page connections and permissions](../relations.md)
- [Page actions](cobalt-page-actions.md) defines the page UI registration and browser evidence.

## Implementation inventory

- `deepwell/src/services/link/backlinks.rs` — permission-filtered read and minimal response types.
- `deepwell/src/services/link/mod.rs` — service exports.
- `deepwell/src/endpoints/link.rs` — request parsing and endpoint.
- `deepwell/src/api.rs` — RPC registration.
- `framerail/src/routes/[slug]/[...extra]/+page.server.ts` — trusted UI action context.

## Tests asserting this spec

- `deepwell/tests/page_backlinks.rs` — isolated transactional target, authorization, privacy, grouping, and read-only cases.
- `framerail/tests/page-backlinks.test.ts` — trusted UI action cases.
- `framerail/tests/local/remaining-actions.mjs` — filtered Backlinks browser presentation.

## Known gaps (current cycle)

- [ ] Final independent checks remain ongoing. The bounded preservation record excludes only the sacrificial Tags/Parent fixture and native cache fields; no full-replica parity claim follows.

## Out of scope

- The existing unfiltered `page_get_links_to` response is unchanged; replacing or restricting it is separate work.
- No mutation endpoint, production deployment, or source-site write is included.
