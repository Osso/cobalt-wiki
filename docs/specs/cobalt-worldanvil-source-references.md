# World Anvil source-reference articles

`tools/cobalt_migration/worldanvil_reference.py` produces a pure private article payload for Wikidot templates, CSS, and admin/system source explicitly kept as reference, not execution. The caller owns selection, classification, source acquisition, identity reconciliation, and all API operations.

## What it must do

- [x] Return a private `article` payload titled `Source reference: <fullname>`, using canonical `fullname` rather than metadata title for identity. Retain original title and fullname in content and the `cobalt-source:<fullname>` identity tag alongside source tags.
- [x] Accept source metadata without `title`; state in content that the original title is unavailable while retaining supplied source text and canonical fullname.
- [x] Retain all source lines, including blank and trailing lines, via explicit `[br]` separators outside `[noparse]` wrappers. Keep Wikidot markup, brackets, and `[/code]` literal; do not turn templates, CSS, or HTML into executable formatting.
- [x] Reject a case-insensitive `[/noparse]` collision in source or identity text instead of guessing an escape.
- [x] HTML-escape `<`, `>`, `&`, and quotes in literal text rather than sending raw HTML markup.
- [ ] Confirm in a saved private article that World Anvil renders escaped HTML characters as the intended visible characters while retaining line breaks and literal BBCode. No browser proof yet; the serializer does not guarantee rendered text fidelity.

## How it works

- World Anvil's [basic BBCode guide](https://www.worldanvil.com/learn/bbcode-tutorials/basic-bbcode) documents `[noparse]…[/noparse]` for displaying BBCode literally and `[br]` for line breaks. It does not establish how HTML entities inside `[noparse]` render for this payload.

## Implementation inventory

- `tools/cobalt_migration/worldanvil_reference.py`: pure source-reference article payload serialization.

## Tests asserting this spec

- `tests/cobalt_migration/test_worldanvil_reference.py`: identity/privacy, canonical-fullname titles, missing-title handling, literal source serialization, and delimiter rejection. The five reference-payload tests passed for `80da2f7c5`; no test run is claimed for this documentation-only update.

## Known gaps (current cycle)

- [ ] Confirm rendered HTML-special-character fidelity in a private destination article before relying on this representation for live migration.

## Out of scope

- Selection/classification, source parsing/acquisition, destination API calls, browser writes, uploads, inventory, operation journals, and migrating any page. See [World Anvil import](cobalt-worldanvil-import.md) for migration-operation contract and evidence.
- Executing Wikidot functionality or guessing undocumented delimiter escaping.
