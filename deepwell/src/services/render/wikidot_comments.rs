//! Wikidot removes `[!-- --]` comments after recognizing same-line `@@raw@@` spans.
//!
//! Cobalt templates rely on that order to hide rows whose value is `@@`:
//! `[!--{$aka}--]@@@@ ...row... [!-- --]` becomes `[!--@@--]@@@@ ...`, where the
//! first `--]` sits inside a raw span, so the comment runs to the closing `[!-- --]`.
//! FTML ends comments at the first `--]`, so comments are removed here first.

const OPEN: &str = "[!--";
const CLOSE: &str = "--]";
const RAW: &str = "@@";

/// Remove comments, skipping raw spans both outside and inside them.
/// Unterminated comments and unpaired raw markers are left as text.
pub(super) fn strip_comments(source: String) -> String {
    if !source.contains(OPEN) {
        return source;
    }
    let mut output = String::with_capacity(source.len());
    let mut index = 0;
    while index < source.len() {
        let rest = &source[index..];
        if rest.starts_with(RAW) {
            let end = raw_span_end(&source, index);
            output.push_str(&source[index..end]);
            index = end;
        } else if rest.starts_with(OPEN)
            && let Some(end) = comment_end(&source, index + OPEN.len())
        {
            index = end;
        } else {
            let length = rest.chars().next().map_or(1, char::len_utf8);
            output.push_str(&rest[..length]);
            index += length;
        }
    }
    output
}

/// End of the raw span opening at `start`, or just past an unpaired marker.
fn raw_span_end(source: &str, start: usize) -> usize {
    let after = start + RAW.len();
    let line_end = source[after..]
        .find('\n')
        .map_or(source.len(), |n| after + n);
    match source[after..line_end].find(RAW) {
        Some(close) => after + close + RAW.len(),
        None => after,
    }
}

/// End of a comment whose body starts at `start`, ignoring `--]` inside raw spans.
fn comment_end(source: &str, start: usize) -> Option<usize> {
    let mut index = start;
    while index < source.len() {
        let rest = &source[index..];
        if rest.starts_with(RAW) {
            index = raw_span_end(source, index);
        } else if rest.starts_with(CLOSE) {
            return Some(index + CLOSE.len());
        } else {
            index += rest.chars().next().map_or(1, char::len_utf8);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROW: &str = "[!--{v}--]@@@@\n|| AKA || {v} ||\n[!-- --]\n|| Gender || Male ||";

    #[test]
    fn a_raw_marker_value_hides_the_guarded_row() {
        let source = ROW.replace("{v}", "@@");
        assert_eq!(strip_comments(source), "\n|| Gender || Male ||");
    }

    #[test]
    fn a_real_value_shows_the_guarded_row() {
        let source = ROW.replace("{v}", "Red");
        assert_eq!(
            strip_comments(source),
            "@@@@\n|| AKA || Red ||\n\n|| Gender || Male ||"
        );
    }

    #[test]
    fn raw_spans_protect_comment_markers_and_unterminated_comments_stay() {
        assert_eq!(strip_comments("@@[!--@@ kept".into()), "@@[!--@@ kept");
        assert_eq!(strip_comments("a [!-- open".into()), "a [!-- open");
        assert_eq!(strip_comments("é[!--x--]ü".into()), "éü");
    }
}
