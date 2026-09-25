# World Anvil source-reference articles

`tools/cobalt_migration/worldanvil_reference.py` produces a pure article payload for Wikidot templates, CSS, and admin/system source selected for reference, not execution. The caller owns classification, source acquisition, identity reconciliation, and all API operations.

## What it must do

- [x] Return a private `article` payload with a title explicitly labeled as a source reference, retaining original title and fullname in content and the `cobalt-source:<fullname>` identity tag alongside source tags.
- [x] Retain all source lines, including blank and trailing lines, via explicit `[br]` separators outside `[noparse]` wrappers. Keep Wikidot markup, brackets, and `[/code]` literal; do not turn templates, CSS, or HTML into executable formatting.
- [x] Reject a case-insensitive `[/noparse]` collision in source or identity text instead of guessing an escape.
- [x] HTML-escape `<`, `>`, `&`, and quotes in literal text rather than sending raw HTML markup.
- [ ] Confirm in a saved private article that World Anvil renders escaped HTML characters as the intended visible characters while retaining line breaks and literal BBCode. No browser proof yet; the serializer does not guarantee rendered text fidelity.

## How it works

- World Anvil's [basic BBCode guide](https://www.worldanvil.com/learn/bbcode-tutorials/basic-bbcode) documents `[noparse]…[/noparse]` for displaying BBCode literally and `[br]` for line breaks. It does not establish how HTML entities inside `[noparse]` render for this payload.

## Implementation inventory

- `tools/cobalt_migration/worldanvil_reference.py`: pure source-reference article payload serialization.

## Tests asserting this spec

- `tests/cobalt_migration/test_worldanvil_reference.py`: identity/privacy, literal source serialization, and delimiter rejection.

## Known gaps (current cycle)

- [ ] Confirm rendered HTML-special-character fidelity in a private destination article before relying on this representation for live migration.

## Out of scope

- Classification, source parsing/acquisition, destination API calls, browser writes, and migrating any page.
- Executing Wikidot functionality or guessing undocumented delimiter escaping.
