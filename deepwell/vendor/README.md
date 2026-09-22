# FTML patch

`ftml/` is the published FTML 1.41.0 crate, verified against the prior Cargo.lock checksum, with local parser/rendering corrections: `RightParentheses` dispatches to text rather than a citation rule requiring `LeftParentheses`; HTML rendering accepts fetched page titles instead of emitting fabricated TODO labels.

Unmatched `))` in acquired source previously panicked during page import. Source bytes remain unchanged; closing delimiters render literally. `tests/page_import.rs` exercises database import and rendered output. `tests/page_link_titles.rs` covers native title resolution and history-preserving rendering refresh.

Cargo and native Nix builds use the same local dependency. Replace this copy with a registry release once that release passes the regression. Upstream license remains in `ftml/LICENSE.md`.
