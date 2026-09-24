# Canonical page names and categories

The replica must preserve existing Wikidot names rather than flattening their colons into new names. Source semantics are documented in the [Wikidot guide](https://community.wikidot.com/howto:getting-started-guide): the first colon separates category from page name; subsequent colons remain part of the name. Two authenticated source pages, with canonical URL and source page ID checked, also show the same writing-category ID for single- and multi-colon names.

## What it must do

- [ ] Preserve multi-colon references and link to the exact target, including when a distinct dash-normalized page exists.
- [ ] Serve and create multi-colon pages at their exact slug, without a redirect to a dash-merged name.
- [ ] Assign imported multi-colon pages to the first-colon category.
- [ ] Preserve ordinary single-colon/default references, explicit labels, fragments and subpaths.
- [x] Reconcile affected existing category assignments without changing source bytes or revision identity. Production, 2026-09-23: 456 imported multi-colon pages moved from 438 per-prefix categories to `writing` (the only affected first segment); the emptied categories, which had no permission rows, were deleted. Only `page.page_category_id` changed. Rollback data: tables `reconcile_20260923_page_category` (page, old category) and `reconcile_20260923_categories` (deleted rows). Afterwards the replica `stats` CountPages totals match Wikidot except one RP log absent from the archive (5,385 vs 5,386).

## How it works

- [Replica status](../wiki/systems/cobalt-replica-status.md)

## Implementation inventory

- `deepwell/vendor/ftml/src/data/page_ref.rs`: preserves colon-separated page-name segments during reference normalization.
- `deepwell/src/utils/category.rs`: first-colon category splitting.
- `deepwell/vendor/wikidot-normalize`: slug normalization keeps later colons (patched crate, see `deepwell/vendor/README.md`).

## Tests asserting this spec

- `deepwell/tests/page_canonical_names.rs`: native category identity and exact links despite a normalized-name collision.
- `deepwell/tests/page_import.rs`: exact import identity and source-category behavior.
- `deepwell/tests/page_multi_colon_slug.rs`: native create and view at the exact slug with no `redirect_page`.
- `deepwell/src/services/view/service.rs` (`multi_colon_page_slug_is_not_redirected`): view redirect normalization.

## Known gaps (current cycle)

- [ ] Full canonical-name corpus proof. The naming code (`f5fbf4c`) is deployed with the rendering branch and existing data is reconciled.
- [ ] Category ACL/creator metadata remains separate; source observations must not be treated as a complete permission export.

## Out of scope

Source rewriting, rename aliases, fabricated source history, and changing unrelated sites' stored data.
