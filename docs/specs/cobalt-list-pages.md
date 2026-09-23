# Cobalt ListPages

Expand archived `[[module ListPages]]` blocks into ordinary wikitext before FTML parsing, matching the 71 uses in the Cobalt archive (homepage, top navigation, rosters, templates). Implementation: `deepwell/src/services/render/list_pages.rs`; deployment status is tracked in the [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

- [x] Recognize `[[module ListPages ...]]...[[/module]]` case-insensitively, including multi-line headers, stray `\` continuations and `]]` inside quoted values; unterminated modules stay text.
- [x] Select by `category` (default current category, `*`, `.`, `+name`, `-name`), `tags`/`tag` (plain = any, `+` = required, `-` = excluded, `-` alone = untagged), `pagetype` (`normal` default excludes `_` pages, `hidden`, `*`) and `created_at="last N days"`.
- [x] Order by `name`, `fullname`, `title`, `created_at` or `updated_at`, `asc` default when a field is given, `created_at desc` when omitted; ties by full name.
- [x] Show `min(limit, perPage)` items, `perPage` defaulting to 20 and capped at 250.
- [x] List only anonymously readable pages, because compiled HTML is shared.
- [x] Fill item tokens: `name`, `fullname`, `title`, `linked_title`/`title_linked` (link with the target's escaped title), `link`, `created_at`/`updated_at` (date block), `form_data{field}`/`form_raw{field}` from the listed page's category form.
- [x] Lay out `separate="yes"` (default) items as `list-pages-item` divs and `separate="no"` items as one block joined by newlines, with `prependLine`/`appendLine`, inside a `list-pages-box` div; a final line continuation does not join the closing div. No items produce nothing.
- [x] Replace a module with unsupported arguments (for example `rssTitle`) with a visible error block naming the problem.
- [ ] Pagination controls beyond the first page.
- [ ] Rerender listing pages when matching pages are created, retagged or deleted.

## Data limits

Imported pages carry import-time `created_at`/`updated_at`; original Wikidot creation dates were not acquired. Ordering and display by those fields ("New Characters", "New Writings", digests) is therefore wrong until original timestamps are imported.

## Tests asserting this spec

- `deepwell/tests/page_list_pages.rs`: category/tag/pagetype filtering, title order, limit, prepended table, form labels, links, anonymous-denial filtering, explicit unsupported-argument error; ignored whole-archive render check.
- `deepwell/src/services/render/list_pages.rs`: header scanning, argument grammar, selection defaults, layouts; ignored `every_archived_header_is_supported` parses all 71 archived headers.

## Out of scope

Other modules (`CountPages`, `NewPage`, `Join`, ...), listing invalidation (queue fan-out is tracked separately), and deployment.
