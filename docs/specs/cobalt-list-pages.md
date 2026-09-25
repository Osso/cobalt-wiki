# Cobalt ListPages

Expand archived `[[module ListPages]]` blocks into ordinary wikitext before FTML parsing, matching the 71 uses in the Cobalt archive (homepage, top navigation, rosters, templates). Implementation: `deepwell/src/services/render/list_pages.rs`; deployment status is tracked in the [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

- [x] Recognize `[[module ListPages ...]]...[[/module]]` case-insensitively, including multi-line headers, stray `\` continuations and `]]` inside quoted values; unterminated modules stay text.
- [x] Select by `category` (default current category, `*`, `.`, `+name`, `-name`), `tags`/`tag` (plain = any, `+` = required, `-` = excluded, `-` alone = untagged; ASCII case-insensitive matching for plain, required and excluded tags, including listing invalidation), `pagetype` (`normal` default excludes `_` pages, `hidden`, `*`) and `created_at="last N days"`.
- [x] Order by `name`, `fullname`, `title`, `created_at` or `updated_at`, `asc` default when a field is given, `created_at desc` when omitted; ties by full name.
- [x] `perPage` defaults to 20, capped at 250.
- [x] List only anonymously readable pages, because compiled HTML is shared.
- [x] Fill item tokens: `name`, `fullname`, `title`, `linked_title`/`title_linked` (link with the target's escaped title), `link`, `created_at`/`updated_at` (date block), `form_data{field}`/`form_raw{field}` from the listed page's category form.
- [x] Lay out `separate="yes"` (default) items as `list-pages-item` divs and `separate="no"` items as one block joined by newlines with `prependLine`/`appendLine` (Wikidot ignores those lines for separate items), inside a `list-pages-box` div; a final line continuation does not join the closing div. No items produce nothing. Joined table rows ending `|| ` render as rows even when followed by another row.
- [x] Replace a module with unsupported arguments (for example `rssTitle`) with a visible error block naming the problem.
- [x] `[[module CountPages ...]]` takes the same selection arguments, counts every matching visible page (no `limit`/`perPage` cap) and renders its body with `%%total%%` filled inside a `list-pages-box` div, as Wikidot does on `stats`.
- [x] Nested modules (an item template containing, or including, another ListPages): the outer module ends at its matching `[[/module]]`; outer tokens fill inner module headers but not inner item templates; inner modules expand after the outer items, up to four levels (Cobalt `testlist`).
- [x] Pagination like Wikidot: `perPage` items per page (`limit` caps items across all pages); a pager (`page N of M`, pages 1–2, current ±2, last two, `...` gaps, previous/next) links to `/<page>/p/N`, which every module on the page follows. Stored HTML is page 1; `/p/N` renders on demand without storing.
- [x] Rerender listing pages when matching pages are created, edited, retagged, moved, deleted or restored: each page body stores its selections (`page_listing`), and a changed page (before and after) queues rerenders of the listing pages whose selection it matches; a matching navigation page also queues a navigation rerender of every page using it, in the same step ([rerender invalidation](cobalt-rerender-invalidation.md)).

## Data limits

Each render loads the site's page metadata (id, category, name, dates, title, tags) once and evaluates every module in memory, so nested listings cost one query; this assumes a site of thousands, not millions, of pages.

Imported pages carry import-time `created_at`/`updated_at`; original Wikidot creation dates were not acquired. Ordering and display by those fields ("New Characters", "New Writings", digests) is therefore wrong until original timestamps are imported.

## Tests asserting this spec

- `deepwell/tests/page_list_pages.rs`: category/tag/pagetype filtering, title order, limit, prepended table, Digest Writings multi-row table, form labels, links, anonymous-denial filtering, explicit unsupported-argument error, CountPages totals beyond one page, nested listing through an include; ignored whole-archive render check.
- `deepwell/src/services/render/list_pages.rs`: header scanning, argument grammar, selection defaults, layouts, joined Digest Writings table through FTML; ignored `every_archived_header_is_supported` parses all 81 archived ListPages/CountPages headers.

## Current repair proof

This is the shared final proof for the `551d29df5`, `62e1467e9`, `1db12295f`, and `73356b625` repairs, including the route/asset clauses in [Canonical page names](cobalt-canonical-names.md) and [Site Manager](cobalt-site-manager.md).

- Independent browser coverage: Applications plus Site Manager passed 2/2 (`/tmp/cobalt-verify-62e1467-browser.log`); Digest passed 1/1 (`/tmp/cobalt-verify-1db12295-browser-digest.log`). Main checkout inspection used `/tmp/claude/cobalt-admin-fixed-{applications,manager,digest}.png`.
- Native checks: `cargo fmt --check` and `cargo check` exited 0 (`/tmp/claude/cobalt-admin-{fmt,check}.log`); the exact ListPages-to-FTML unit passed 1/1 (`/tmp/claude/cobalt-admin-list-pages-ftml.log`), and the isolated Redis-0 database Digest test passed 1/1 (`/tmp/claude/cobalt-admin-digest-db-isolated-redis0.log`).
- Parser fixture runner covered 122/125 cases; advanced table and simple-table cases passed. The three remaining expected paths are unrelated basic audio, image and video inputs that omit `test:` (`/tmp/claude/cobalt-admin-ftml-table-fixtures-nocapture.log`).
- Local `./deploy.sh` succeeded at `1db12295f`; Digest rerender snapshots preserved page-view source and revision identity (`/tmp/claude/cobalt-admin-digest-{before,after}.json`).
- Frontend follow-up: ESLint, Stylelint and Prettier exited 0. `svelte-check` retained only five pre-existing `imported-history.mjs` errors at lines 87, 103, 110, 158 and 164; the new Admin-test errors were gone.

The Applications dashboard still exposes a separate imported-metadata gap: `application:badchemistry` has `updated_at = null`, so the page renders literal `%%updated_at%%`; no date was fabricated. These fixes have no public deployment and do not establish full replica or legacy-admin completeness.

## Out of scope

Other modules (`CountPages`, `NewPage`, `Join`, ...), listing invalidation (queue fan-out: [rerender invalidation](cobalt-rerender-invalidation.md)), and deployment.
