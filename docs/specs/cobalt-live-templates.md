# Cobalt live form templates

Render archived Wikidot form pages through their category `_template`, as Wikidot does, without rewriting the stored YAML record. Implementation: `deepwell/src/services/render/live_template.rs` and `page_tokens.rs`; deployment status is tracked in the [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

- [x] When a page's category `_template` (or `_template` for the default category) is anonymously readable and contains one `[[form]]` definition, render the template text before its first `====` line instead of the page source.
- [x] Substitute `%%form_data{field}%%` (select option label; other fields their stored value), `%%form_raw{field}%%` (stored value), `%%title%%`, `%%name%%`, `%%fullname%%`; missing fields become empty and unknown tokens stay literal.
- [x] Leave ListPages item templates inside the template untouched, so their tokens describe listed pages; module headers still receive this page's tokens (`tags="+%%name%%"`).
- [x] Substitute before include expansion, so template includes receive filled arguments.
- [x] Keep hidden pages (`_template`, `_public`) and pages in categories without a form template rendered from their own source.
- [x] Fail the render explicitly for malformed form markers/definitions or a form page whose source is not a field record.
- [ ] Rerender category pages when their `_template` changes.
- [ ] Non-form live templates (`%%content%%`); none exist in the Cobalt archive.

## How it works

Rendering order for a published page body: live template → includes ([spec](cobalt-includes.md)) → ListPages ([spec](cobalt-list-pages.md)) → FTML. Include argument values are trimmed like Wikidot, because the archived templates pass one argument per line.

Archive check (2026-09-22): 5,969 of 5,970 pages in the eight form categories parse as field records; the exception, `player:_public`, is Wikidot's plain-text non-member page and is hidden, so it is never wrapped.

## Tests asserting this spec

- `deepwell/tests/page_list_pages.rs`: `form_pages_render_through_their_category_template` (select label, wiki field, filled include, unchanged YAML source, unwrapped `_public`); ignored `archived_cobalt_pages_render_without_template_syntax` renders real archived pages.
- `deepwell/src/services/render/page_tokens.rs`, `live_template.rs`: token substitution, `====` split and ListPages exclusion.
- `deepwell/vendor/ftml/src/includes/test.rs`: `argument_values_exclude_whitespace_before_the_next_separator`.

## Out of scope

Template-change invalidation (queue fan-out is tracked separately), form editing (see [form schema](cobalt-data-form-schema.md)), and deployment.
