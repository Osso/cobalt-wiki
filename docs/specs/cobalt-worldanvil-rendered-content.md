# Rendered page content for World Anvil

`tools/cobalt_migration/worldanvil_html.py` converts already-rendered source HTML into World Anvil BBCode. The caller owns acquisition, page identity, content selection upstream, image uploads, and all destination mutations. This converter selects only the exact `div#page-content` subtree.

## What it must do

- [x] Exclude surrounding site chrome and reject missing, duplicate, or unclosed content wrappers.
- [x] Preserve visible text and HTML entities with paragraphs, headings, breaks, inline emphasis, quotes, lists, rules, tables, code, and transparent divisions/spans.
- [x] Resolve links and image URLs against the source page; emit uploaded image IDs only when the optional resolver supplies a positive ID, otherwise retain the explicit absolute source URL.
- [x] Render observed Wikidot YUI tabsets as ordered static labeled sections, including panels hidden by the tab widget; recognize only the paired library script, navigation, panels, and matching initializer.
- [x] Block unknown elements, scripts/forms/frames, unmatched tabs, hidden content outside recognized panels, cell spans, malformed or unsafe URLs, invalid image references, and literal BBCode brackets rather than dropping them.

## How it works

- No separate architecture document; conversion is a pure local operation.

## Implementation inventory

- `tools/cobalt_migration/worldanvil_html.py`: wrapper parsing, strict rendered-content conversion, YUI tab extraction.

## Tests asserting this spec

- `tests/cobalt_migration/test_worldanvil_html.py`: synthetic formatting, links/images, void tags, nested tables, YUI tabs, and rejection boundaries.

## Known gaps (current cycle)

- [ ] General source-rendered dynamic modules other than the observed YUI tabview remain unsupported.

## Out of scope

- Fetching, selecting article fields, checking identity, uploading images, creating or modifying articles: main migration owns these.
- Site-wide CSS reproduction or executing source scripts: static content only.
- Literal bracket escaping: conversion blocks bracket-bearing source text until a verified World Anvil literal syntax is available.
