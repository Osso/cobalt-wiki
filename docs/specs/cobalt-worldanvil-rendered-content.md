# Rendered page content for World Anvil

`tools/cobalt_migration/worldanvil_html.py` converts already-rendered source HTML into World Anvil BBCode. The caller owns acquisition, page identity, content selection upstream, image uploads, and all destination mutations. This converter selects only the exact `div#page-content` subtree.

## What it must do

- [x] Exclude surrounding site chrome and reject missing, duplicate, or unclosed content wrappers.
- [x] Preserve visible text and HTML entities with paragraphs, headings, breaks, inline emphasis, quotes, lists, rules, tables, code, and transparent divisions/spans.
- [x] Resolve links and image URLs against the source page; emit uploaded image IDs only when the optional resolver supplies a positive ID, otherwise retain the explicit absolute source URL.
- [x] Render observed Wikidot YUI tabsets as ordered static labeled sections, including panels hidden by the tab widget; recognize only the paired library script, navigation, panels, and matching initializer.
- [x] Render observed Wikidot collapsible blocks as `[spoiler=label]...[/spoiler]`, preserving labels and nested content; accept only the folded/unfolded controls and hidden panel structure.
- [x] Preserve literal bracket-bearing source text inside `[noparse]...[/noparse]` (also via `literal_text` for source-reference text); reject any text with a `[/noparse]` collision. Keep generated BBCode structural.
- [x] Preserve children of inert `href="javascript:;"` anchors without emitting navigation; reject actionable event attributes and other JavaScript URLs.
- [x] Block unknown elements, scripts/forms/frames (including YouTube embeds), unmatched tabs or collapsibles, hidden content outside recognized panels, cell spans, malformed or unsafe URLs, and invalid image references rather than dropping them.

## How it works

- No separate architecture document; conversion is a pure local operation.

## Implementation inventory

- `tools/cobalt_migration/worldanvil_html.py`: wrapper parsing, strict rendered-content conversion, YUI tab and collapsible extraction, literal text helper.

## Tests asserting this spec

- `tests/cobalt_migration/test_worldanvil_html.py`: formatting, links/images, void tags, nested tables, YUI tabs, nested collapsibles, literal brackets, and rejection boundaries.

## Known gaps (current cycle)

- [ ] General source-rendered dynamic modules other than the observed YUI tabview remain unsupported.

## Out of scope

- Fetching, selecting article fields, checking identity, uploading images, creating or modifying articles: main migration owns these.
- Site-wide CSS reproduction or executing source scripts: static content only.
- Arbitrary BBCode markup in source-reference titles or collapsible labels: labels containing brackets still block because `[noparse]` cannot safely be placed inside a spoiler parameter.
