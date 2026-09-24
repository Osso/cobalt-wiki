# Cobalt live form templates

Render archived Wikidot form pages through their category `_template`, as Wikidot does, without rewriting the stored YAML record. Implementation: `deepwell/src/services/render/live_template.rs` and `page_tokens.rs`; deployment status is tracked in the [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

- [x] When a page's category `_template` (or `_template` for the default category) is anonymously readable and contains one `[[form]]` definition, render the template text before its first `====` line instead of the page source.
- [x] Substitute `%%form_data{field}%%` (select option label or stored value, shown as literal text like Wikidot's raw span; `wiki` fields as wikitext), `%%form_raw{field}%%` (stored value as wikitext), `%%title%%`, `%%name%%`, `%%fullname%%`; missing fields become empty and unknown tokens stay literal.
- [x] A `form_data` raw span used as a triple-link label shows its text (`[[[player:{$author}|{$author}]]]` → `Alli`, as on Wikidot); in a table cell it stays literal (Wikidot's `pre-wrap` span, checked on writing:2025-08-20-time-for-questions). Fixture `deepwell/vendor/ftml/test/link/triple-raw` (`c370654`).
- [x] Leave ListPages item templates inside the template untouched, so their tokens describe listed pages; module headers still receive this page's tokens (`tags="+%%name%%"`).
- [x] Substitute before include expansion, so template includes receive filled arguments.
- [x] Keep hidden pages (`_template`, `_public`) and pages in categories without a form template rendered from their own source.
- [x] Render an explicit diagnostic for malformed form markers/definitions or a form page whose source is not a field record, without exposing its raw source or changing stored source.
- [x] Queue only the affected category's pages for rerender when its `_template` changes, through the normal edit path without changing their records.
- [x] Deliver queued rerenders through the local worker and refresh their compiled bodies within five seconds after a normal UI template edit.
- [ ] Non-form live templates (`%%content%%`); none exist in the Cobalt archive.

## How it works

Rendering order for a published page body: live template → includes ([spec](cobalt-includes.md)) → ListPages ([spec](cobalt-list-pages.md)) → FTML. Include argument values are trimmed like Wikidot, because the archived templates pass one argument per line.

Archive check (2026-09-22): 5,969 of 5,970 pages in the eight form categories parse as field records; the exception, `player:_public`, is Wikidot's plain-text non-member page and is hidden, so it is never wrapped.

## Tests asserting this spec

- `deepwell/tests/page_list_pages.rs`: `form_pages_render_through_their_category_template` (select label, wiki field, filled include, unchanged YAML source, unwrapped `_public`); ignored `archived_cobalt_pages_render_without_template_syntax` renders real archived pages.
- `deepwell/src/services/render/page_tokens.rs`, `live_template.rs`: token substitution, `====` split and ListPages exclusion.
- `deepwell/vendor/ftml/src/includes/test.rs`: `argument_values_exclude_whitespace_before_the_next_separator`.
- `deepwell/tests/page_listing_invalidation.rs`: `editing_a_form_template_queues_only_its_category_without_changing_records` proves category-scoped queueing through a normal template edit; it passed against the dedicated empty Redis 15/test database at `72743dc` (`/tmp/claude/cobalt-template-edit-native.log`). The worker cannot observe this rollback-isolated fixture's private transaction, so compiled-body refresh remains unproven.
- `framerail/tests/local/template-refresh.mjs` is the local browser acceptance test: revision `7d379f5` requires dependent compiled HTML to refresh within five seconds. Its earlier 30-second run failed at revision `476b5c0` (`/tmp/claude/cobalt-template-refresh-browser.log`), before the polling change, and is insufficient to judge the revised local runtime. The revised local runtime passed 1/1 at `cc5da0a` after local deploy of `0aa9975` (`/tmp/claude/cobalt-template-refresh-browser-fast.log`): a normal UI template edit produced the dependent stored compiled `After` marker through the background worker in 1,865 ms. The dependent YAML/current revision and an unrelated page's source, revision, and compiled body remained unchanged; a UI reload displayed `After`.
- Revision `0aa9975` caps empty-queue exponential backoff with saturating doubling. The existing local preview runtime is configured for two workers and a one-to-two-second empty-queue poll (`/home/osso/.local/share/cobalt-wiki/local-full/deepwell.toml`); production configuration is unchanged. Synthetic fixtures remain in use. This proves local worker refresh, not full replica readiness or source-parity behavior.

## Out of scope

Form editing (see [form schema](cobalt-data-form-schema.md)) and deployment.
