# FTML patch

`ftml/` is the published FTML 1.41.0 crate, verified against the prior Cargo.lock checksum, with one parser correction: `RightParentheses` dispatches to text, not the bibliography-citation rule that requires `LeftParentheses`.

Unmatched `))` in acquired source previously panicked during page import. Source bytes remain unchanged; closing delimiters render literally. `tests/page_import.rs` exercises database import and rendered output.

Cargo and native Nix builds use the same local dependency. Replace this copy with a registry release once that release passes the regression. Upstream license remains in `ftml/LICENSE.md`.
