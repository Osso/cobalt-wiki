# World Anvil source-reference articles

`tools/cobalt_migration/worldanvil_reference.py` produces a private article payload for Wikidot templates, CSS, and admin/system source retained as reference, never execution. The caller owns selection, classification, source acquisition, identity reconciliation, and API operations.

## What it must do

- [x] Return a private `article` payload titled `Source reference: <fullname>` using canonical `fullname`, not metadata title, for identity. Retain original title and fullname in the explanatory body and add `cobalt-source:<fullname>` with source tags.
- [x] Accept metadata without `title`; the body states that the original title is unavailable.
- [x] Store the exact supplied raw source string, unchanged, in editable native `authornotes`. This includes blank/trailing lines, markup, HTML, and parser delimiters such as `[/noparse]`.
- [x] Keep the private article body explanatory only: it states that the source is in Author's Notes and is not rendered because body rendering changes source text.
- [x] Reject a `[/noparse]` collision only in metadata rendered through the body literal serializer; do not apply that rendered-body limitation to `authornotes` source.
- [x] Read back `authornotes` after creation. A missing or changed value leaves the journal entry `created_unverified` and blocks a false success.

## Implementation inventory

- `tools/cobalt_migration/worldanvil_reference.py`: pure source-reference payload serialization.
- `tools/cobalt_migration/worldanvil_import.py`: create readback, including `authornotes` when supplied in the payload.

## Tests asserting this spec

- `tests/cobalt_migration/test_worldanvil_reference.py`: five reference-payload tests cover canonical identity, private metadata body, exact Author's Notes storage, delimiter safety, missing metadata title, and rendered metadata rejection.
- `tests/cobalt_migration/test_worldanvil_import.py`: one targeted missing-Author's-Notes readback test prevents a verified result.
- These targeted tests passed for the current change; this documentation update performs no API operation or destination write.

## Known gaps

- [ ] Browser proof of the editable Author's Notes UI is not recorded here; API readback establishes stored payload equality, not UI rendering behavior.

## Out of scope

- Executing Wikidot functionality, rendering source in the article body, selection/classification, source parsing/acquisition, uploads, inventory, browser writes, and migration of any page. See [World Anvil import](cobalt-worldanvil-import.md).
