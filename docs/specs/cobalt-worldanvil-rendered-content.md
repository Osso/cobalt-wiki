# Rendered page content for World Anvil

`tools/cobalt_migration/worldanvil_html.py` converts the exact rendered `div#page-content` subtree into World Anvil BBCode. The caller owns acquisition, identity, image uploads, and destination mutation.

## Supported conversion matrix

| Source structure | Output / boundary |
| --- | --- |
| Paragraphs, headings, breaks, emphasis, quotes, lists, rules, tables, code, transparent `div`/`span` | Native BBCode while preserving visible text and HTML entities. |
| Links | Resolved absolute URL with rendered link text. |
| Images | `[img:ID]` when the optional resolver returns a positive ID; otherwise `[img:absolute-source-url]`. |
| Observed YUI tabsets | Ordered labeled static sections, including recognized hidden panels. |
| Observed collapsible blocks | Native `[spoiler]body\|label[/spoiler]`. The body and label may not contain `|`; nested collapsibles therefore remain unsupported. |
| Literal bracket-bearing text | `[noparse]...[/noparse]`; a `[/noparse]` collision blocks conversion. |
| Observed tag-editor/publish controls | Retain visible label only; no action/navigation. All other actionable attributes or JavaScript URLs block. |
| Titled YouTube embed iframe | Exact source URL as `[url:exact-source]title[/url]`; a link, not an embedded player. Only observed 11-character `/embed/` URLs, optionally with `?si=`, are accepted. |

## What it must reject

- Missing, duplicate, or unclosed page-content wrappers; surrounding site chrome.
- Unknown elements, scripts, forms, unsupported frames, malformed or unsafe URLs, invalid image resolver output, unmatched tabs/collapsibles, hidden content outside recognized panels, and table cell spans.
- Spoiler labels or bodies with a pipe, nested spoilers, and arbitrary BBCode in spoiler labels.

## Implementation inventory

- `tools/cobalt_migration/worldanvil_html.py`: strict parser, formatting conversion, links/images, YUI tabs and collapsibles, inert controls, and YouTube link conversion.

## Tests asserting this spec

- `tests/cobalt_migration/test_worldanvil_html.py`: formatting, links/images, tables, tabs, native spoiler order, pipe/nested-spoiler rejection, literal text, observed controls, YouTube links, and rejection boundaries.

## Known gaps

- [ ] Dynamic source modules outside the observed YUI tabview and collapsible structures remain unsupported.
- [ ] The converter is not yet wired across the complete migration inventory.

## Out of scope

- Fetching/selecting source fields, identity checks, uploads, article writes, site-wide CSS reproduction, and executing source scripts. See [World Anvil import](cobalt-worldanvil-import.md).
