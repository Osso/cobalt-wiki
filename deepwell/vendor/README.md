# FTML patch

`ftml/` is the published FTML 1.41.0 crate, verified against the prior Cargo.lock checksum, with local parser/rendering corrections: `RightParentheses` dispatches to text rather than a citation rule requiring `LeftParentheses`; HTML rendering accepts fetched page titles instead of emitting fabricated TODO labels; table parsing treats whitespace between a closing `||` and row-ending line break, paragraph break, or input end as part of that row ending.

Unmatched `))` in acquired source previously panicked during page import. Source bytes remain unchanged; closing delimiters render literally. `tests/page_import.rs` exercises database import and rendered output. `tests/page_link_titles.rs` covers native title resolution and history-preserving rendering refresh. `tests/page_list_pages.rs` covers the consumer-facing Digest Writings table with trailing whitespace after the closing delimiter.

The lexer now tries existing whitespace rules immediately after raw/comment delimiters, before text and symbols. This preserves token priority for overlapping markup while reducing repeated ordered-choice checks on whitespace-heavy pages. A standalone debug Pest benchmark with identical baseline and reordered grammars matched token rules, slices, and byte spans; median lexer time fell from 724 to 430 ms on ~60 KB and 2,958 to 1,168 ms on ~121 KB. These are tokenizer-only measurements, not preview latency or timeout proof.

Cargo and native Nix builds use the same local dependency. Replace this copy with a registry release once that release passes the regression. Upstream license remains in `ftml/LICENSE.md`.

# wikidot-normalize patch

`wikidot-normalize/` is the published 0.12.0 crate without `merge_multi_categories`, applied to Deepwell and FTML through `[patch.crates-io]`, and to wws through its own `[patch.crates-io]` path to this directory. Upstream rewrote `a:b:c` to `a-b:c`; Wikidot's `WDStringUtils::toUnixName` keeps later colons in the page name, and imported slugs store them. The merge made `page_view` return `redirect_page` `writing-…:…` for stored multi-colon pages, which Framerail followed into a 404. Remove this copy if upstream drops the merge. Upstream license remains in `wikidot-normalize/LICENSE.md`.
