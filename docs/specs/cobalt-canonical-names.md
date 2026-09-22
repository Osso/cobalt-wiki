# Canonical page names and categories

The replica must preserve existing Wikidot names rather than flattening their colons into new names. Source semantics are documented in the [Wikidot guide](https://community.wikidot.com/howto:getting-started-guide): the first colon separates category from page name; subsequent colons remain part of the name. Two authenticated source pages, with canonical URL and source page ID checked, also show the same writing-category ID for single- and multi-colon names.

## What it must do

- [ ] Preserve multi-colon references and link to the exact target, including when a distinct dash-normalized page exists.
- [ ] Assign imported multi-colon pages to the first-colon category.
- [ ] Preserve ordinary single-colon/default references, explicit labels, fragments and subpaths.
- [ ] Reconcile affected existing category assignments without changing source bytes or revision identity.

## How it works

- [Replica status](../wiki/systems/cobalt-replica-status.md)

## Implementation inventory

- `deepwell/vendor/ftml/src/data/page_ref.rs`: preserves colon-separated page-name segments during reference normalization.
- `deepwell/src/utils/category.rs`: first-colon category splitting.

## Tests asserting this spec

- `deepwell/tests/page_canonical_names.rs`: native category identity and exact links despite a normalized-name collision.
- `deepwell/tests/page_import.rs`: exact import identity and source-category behavior.

## Known gaps (current cycle)

- [ ] Native tests, full canonical-name corpus proof, deployment and existing-data reconciliation.
- [ ] Category ACL/creator metadata remains separate; source observations must not be treated as a complete permission export.

## Out of scope

Source rewriting, rename aliases, fabricated source history, and changing unrelated sites' stored data.
