# Cobalt includes

Expand archived Wikidot includes before FTML rendering without rewriting stored source. Implementation lives in `deepwell/src/services/render/includes.rs`; current deployment and local proof are tracked in the [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

- [x] Expand nested same-site includes and substitute supplied variables while preserving stored source bytes.
- [ ] Apply expansion to page body and both navigation regions.
- [ ] Never insert missing, deleted, foreign-site, or anonymously unreadable target content into shared compiled HTML.
- [ ] Record included-page dependencies so later source changes can invalidate compiled output.
- [ ] Terminate cyclic or excessive expansion with an explicit error.

## How it works

- [Replica status and local development workflow](../wiki/systems/cobalt-replica-status.md)

## Implementation inventory

- `deepwell/vendor/ftml/src/includes/mod.rs`: existing parser exposed for asynchronous source resolution; variable substitution remains in FTML.
- `deepwell/src/services/render/includes.rs`: same-site source resolution and bounded expansion for shared output.
- `deepwell/src/services/render/service.rs`: preprocessing and dependency integration.

## Tests asserting this spec

- `deepwell/tests/page_includes.rs`: nested substitution and unchanged stored source.
- `deepwell/vendor/ftml/src/includes/test.rs`: scanner and substitution contracts.

## Known gaps (current cycle)

- [ ] Verify missing/denied targets, navigation, dependencies, and expansion limits with native fixtures.
- [ ] Verify actual archived homepage/navigation includes in the local browser.
- [ ] Reconcile parser coverage against source syntax, including currently unrecognized directives.
- [ ] Establish source ACL equivalence and invalidation after permission changes; target anonymous-read checks alone do not prove either.
- [ ] Support viewer-private includes through viewer-aware rendering; shared output cannot encode viewer-specific decisions.
- [ ] Acquire authorized foreign include sources and establish their rendering/permission behavior.

## Out of scope

No source mutation, implicit cross-site network fetch, or production deployment during this local integration slice. Full replica requirements remain open.
