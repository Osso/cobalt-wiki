# Cobalt includes

Expand archived Wikidot includes before FTML rendering without rewriting stored source. Implementation lives in `deepwell/src/services/render/includes.rs`; current deployment and local proof are tracked in the [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

- [x] Expand nested same-site includes, including empty values, and substitute supplied variables while preserving stored source bytes.
- [x] Ignore pipe segments without `=` (as Wikidot does) instead of dropping the whole directive.
- [x] Apply expansion to page body and both navigation regions.
- [x] Never insert missing, deleted, foreign-site, or anonymously unreadable target content into shared compiled HTML.
- [x] Record only resolved same-site included-page dependencies; unavailable and foreign directives create none.
- [x] Omit `:snippets:suo` `type=showto` regions (with their markers) from shared output; an unterminated region hides the rest of the source.
- [x] Show `:snippets:suo` `type=showto` regions to the signed-in users they list (`user`, `user0`..`user99`, compared case-insensitively with the session user's slug) and to nobody else. As on Wikidot (the snippet wraps the region in `ListUsers users="."`), anonymous visitors get nothing; unlike Wikidot, which sends the region to every signed-in user and hides it with CSS, unlisted users are never sent it. The page body, top bar or side bar whose source lists the viewer is rendered for them at view time without storing (`page_view` session only); everyone else gets the shared stored HTML.
- [ ] Invalidate compiled output after included-page or permission changes.
- [x] Terminate cyclic expansion with an explicit error without replacing the stored compiled revision.
- [x] Accept terminal output at depth 16; reject an additional nesting level without replacing the stored compiled revision.
- [ ] Reject excessive directive/output size before allocation; current output-size guard is post-expansion.

## How it works

- [Replica status and local development workflow](../wiki/systems/cobalt-replica-status.md)

## Implementation inventory

- `deepwell/vendor/ftml/src/includes/mod.rs`: existing parser exposed for asynchronous source resolution; variable substitution remains in FTML.
- `deepwell/src/services/render/includes.rs`: same-site source resolution and bounded expansion for shared output.
- `deepwell/src/services/render/service.rs`: preprocessing and dependency integration.
- `deepwell/src/services/render/show_to.rs`, `ViewService::page`, `PageRevisionService::render_show_to_view`: show-to stripping and per-viewer rendering.

## Tests asserting this spec

- `deepwell/tests/page_show_to.rs`: anonymous, unlisted-member and listed viewers of a page whose body and top bar have show-to regions, plus a missing page's top bar; anonymous and unlisted output contain no restricted text, before and after a listed viewer's render.
- `deepwell/src/services/render/show_to.rs` unit tests: stripping, unterminated regions, listed/unlisted/partial names, per-region reveal.
- `deepwell/tests/page_includes.rs`: six native cases for nested body substitution, both navigation regions, empty values, unavailable/denied/foreign isolation, foreign dependency exclusion, and cycle/depth preservation.
- `deepwell/vendor/ftml/src/includes/test.rs`: six focused scanner/substitution cases including empty values and ignored segments without `=`.

## Known gaps (current cycle)

- [ ] Verify included-page and permission-change invalidation plus pre-allocation size limits.
- [ ] Verify actual archived homepage/navigation includes in the local browser; these still require source-grammar work.
- [ ] Reconcile parser coverage against source syntax, including nav directives the current scanner does not parse.
- [ ] Establish source ACL equivalence and invalidation after permission changes; target anonymous-read checks alone do not prove either.
- [ ] Support viewer-private includes through viewer-aware rendering; shared output cannot encode viewer-specific decisions. Show-to regions are revealed only in the page's or nav page's own source; regions inside live templates or included pages stay hidden from everyone.
- [ ] Acquire authorized foreign include sources and establish their rendering/permission behavior.

## Out of scope

No source mutation, implicit cross-site network fetch, or production deployment during this local integration slice. Full replica requirements remain open.
