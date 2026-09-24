# Additive World Anvil import

`tools/cobalt_migration/worldanvil_import.py` provides create-only import primitives. The requested migration covers **all missing Cobalt wiki pages**, while preserving every existing destination article, including manually edited player and character profiles. These primitives do not yet implement that full conversion scope.

## What it must do

- [x] Preserve existing same-name articles without issuing a creation request.
- [x] Record a pending attempt before creation; an uncertain result cannot trigger an automatic second creation.
- [x] Record the returned ID before readback; mismatched content remains unverified rather than being declared imported.
- [x] Preserve text-only player biography, RP/contact preferences, and sidebar fields in private articles.
- [x] Block unsupported player markup, portraits, and unknown nonempty fields rather than silently discarding content.
- [ ] Cover every source category, including character/background-character forms, writings, reference pages, templates, and dynamic pages; report unrepresentable cases explicitly.
- [ ] Resolve source aliases against destination identity evidence before selecting additions. User-confirmed `player:ozmaasimov` refers to the existing Ozma article, not a missing profile.
- [ ] Preserve media and links; API limitations are blockers, not permission to omit attachments.
- [ ] Verify all additions by live readback and compare pre-existing destination pages against a saved baseline.

## How it works

- [Create-only transport contract](cobalt-worldanvil-client.md)
- Source identities come from [page metadata](cobalt-page-metadata.md) and the [listing export](cobalt-listing-export.md); page content comes from the native backup.
- Journal files are atomically written with owner-only permissions outside the repository. A pending or unverified creation requires reconciliation, not another create request.

## Implementation inventory

- `tools/cobalt_migration/worldanvil_import.py`: text-only player payload conversion, last-moment identity check, creation journal, readback checks.
- `tools/cobalt_migration/worldanvil_client.py`: read and create transport; no update/delete operation.

## Tests asserting this spec

- `tests/cobalt_migration/test_worldanvil_import.py`: profile content/privacy, explicit unsupported-source failures, preservation, uncertain-response resume, readback mismatch.
- `tests/cobalt_migration/test_worldanvil_client.py`: local HTTP pagination and create-only effects.

## Known gaps (current cycle)

- [ ] Character, writing, and other page conversions are not implemented here.
- [ ] Image upload is documented as unavailable through Boromir; authenticated web access is needed to establish an alternative supported workflow.
- [ ] No ongoing synchronization is implemented.

## Out of scope

- Updating or deleting pre-existing articles: explicitly prohibited by the user.
- Deploying the local Cobalt replica or installing a change-sync hook: separate work.
