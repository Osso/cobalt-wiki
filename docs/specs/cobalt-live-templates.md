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

### Rendering budgets

Dependency resolution (live templates, includes, and ListPages) must not consume the FTML-only preprocessing allowance. Since `6074af7`, dependency resolution runs outside that FTML guard, while one cooperative total deadline covers dependency resolution, FTML preprocessing, parsing, and rendering. The configured preprocessing and parse/render limits remain unchanged; the aggregate deadline is their sum. Exceeding either applicable deadline reports `RenderTimeout`; preview must preserve stored pages, revisions, and source, and leave its database transaction usable. These async deadlines do not preempt synchronous CPU work or immediately cancel an in-flight PostgreSQL query.

Tokenizer profiling (2026-09-24): an actual Atley preview exceeded the unchanged 2.5-second aggregate deadline at 2,938 ms; its 134,855-byte preprocessed input produced 51,987 tokens and spent ~1,338 ms in lexing, versus ~229 ms in SQL. A controlled exact-grammar candidate put whitespace alternatives earlier and preserved token rules, slices, and UTF-8 byte spans on overlapping public inputs plus five actual wiki fields (60,532 bytes; doubled to 121,064 bytes). Its debug medians improved from 724 to 430 ms and from 2,958 to 1,168 ms, but it is not an exact expanded end-to-end preview benchmark. The production-grammar targeted token test passed 1/1 at `5659f51` with 132 total tests (one selected, 131 filtered) and a fixture in the existing token test. This establishes lexer token identity for the ordering change, not that Atley now meets its deadline; the configured limits remain unchanged.

Historical failure evidence is retained: concurrent normal previews at `bd98348`/`aef1b8e` returned Player successfully in 548 ms but Character with backend `page_preview` `code_trace` `[1001, 1202]` in 650 ms (`/tmp/claude/cobalt-preview-concurrent-code-trace.log`). This was not established as browser polling or CPU-timing failure. The native `16d5f17` regression put a real transaction-local PostgreSQL dependency read (`pg_sleep(500ms)`) under a 150 ms preprocessing allowance and reproduced the old `RenderTimeout` (`/tmp/claude/cobalt-preview-dependency-red-native.log`). The traced root cause was asynchronous dependency database resolution inside the former 500 ms FTML preprocessing guard.

## Tests asserting this spec

- `deepwell/tests/page_preview.rs`: a transaction-local delayed include read exceeds the preprocessing allowance but must render within the total budget; an over-budget dependency must fail without writes and permit subsequent database reads. `16d5f17` reproduced the old failure (`/tmp/claude/cobalt-preview-dependency-red-native.log`). The two corrected native cases are green on `6074af7`; deployment plus browser/static acceptance gates remain pending, so this is not final end-to-end preview readiness.

- `deepwell/tests/page_list_pages.rs`: `form_pages_render_through_their_category_template` (select label, wiki field, filled include, unchanged YAML source, unwrapped `_public`); ignored `archived_cobalt_pages_render_without_template_syntax` renders real archived pages.
- `deepwell/src/services/render/page_tokens.rs`, `live_template.rs`: token substitution, `====` split and ListPages exclusion.
- `deepwell/vendor/ftml/src/includes/test.rs`: `argument_values_exclude_whitespace_before_the_next_separator`.
- Existing vendored FTML token test: the new whitespace-ordering fixture asserts exact token rule, source slice, and UTF-8 byte span; the production grammar target passed 1/1 at `5659f51` (`/tmp/claude/cobalt-whitespace-5659f51-token-manifest-tests.log`; 132 total, one selected). The initial `-p ftml` invocation was not a workspace member and is retained as setup evidence, not a RED result. Native compilation required rebuilding after host Rust 1.98.1 differed from an existing Rust 1.97.1 artifact; it completed in 5m6s without dependency, timeout, or `cargo clean` changes.
- `deepwell/tests/page_listing_invalidation.rs`: `editing_a_form_template_queues_only_its_category_without_changing_records` proves category-scoped queueing through a normal template edit; it passed against the dedicated empty Redis 15/test database at `72743dc` (`/tmp/claude/cobalt-template-edit-native.log`). The worker cannot observe this rollback-isolated fixture's private transaction, so compiled-body refresh remains unproven.
- `framerail/tests/local/template-refresh.mjs` is the local browser acceptance test: revision `7d379f5` requires dependent compiled HTML to refresh within five seconds. Its earlier 30-second run failed at revision `476b5c0` (`/tmp/claude/cobalt-template-refresh-browser.log`), before the polling change, and is insufficient to judge the revised local runtime. The revised local runtime passed 1/1 at `cc5da0a` after local deploy of `0aa9975` (`/tmp/claude/cobalt-template-refresh-browser-fast.log`): a normal UI template edit produced the dependent stored compiled `After` marker through the background worker in 1,865 ms. The dependent YAML/current revision and an unrelated page's source, revision, and compiled body remained unchanged; a UI reload displayed `After`.
- Revision `0aa9975` caps empty-queue exponential backoff with saturating doubling. The existing local preview runtime is configured for two workers and a one-to-two-second empty-queue poll (`/home/osso/.local/share/cobalt-wiki/local-full/deepwell.toml`); production configuration is unchanged. Synthetic fixtures remain in use. This proves local worker refresh, not full replica readiness or source-parity behavior.

## Out of scope

Form editing (see [form schema](cobalt-data-form-schema.md)) and deployment.
