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
            if output.ends_with('\n') {
                output.pop();
            }
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
            "@@@@\n|| AKA || Red ||\n|| Gender || Male ||"
        );
    }

    #[test]
    fn raw_spans_protect_comment_markers_and_unterminated_comments_stay() {
        assert_eq!(strip_comments("@@[!--@@ kept".into()), "@@[!--@@ kept");
        assert_eq!(strip_comments("a [!-- open".into()), "a [!-- open");
        assert_eq!(strip_comments("é[!--x--]ü".into()), "éü");
    }

    #[test]
    fn stripping_comments_consumes_one_preceding_newline_without_touching_raw_or_utf8() {
        assert_eq!(
            strip_comments("é\n[!-- guard --]@@@@\nCaption".into()),
            "é@@@@\nCaption"
        );
        assert_eq!(
            strip_comments("Caption\n[!-- --]\nTail".into()),
            "Caption\nTail"
        );
        assert_eq!(
            strip_comments("é@@[!-- raw --]@@\n[!-- end --]z".into()),
            "é@@[!-- raw --]@@z"
        );
    }

    #[test]
    fn included_imagebox_caption_has_one_break_without_paragraph_wrapper() {
        use ftml::data::{PageInfo, PageRef, ScoreValue};
        use ftml::includes::{FetchedPage, IncludeRef, Includer};
        use ftml::layout::Layout;
        use ftml::render::{Render, html::HtmlRender};
        use ftml::settings::{WikitextMode, WikitextSettings};
        use std::borrow::Cow;
        use std::convert::Infallible;

        struct ImageBox;
        impl<'t> Includer<'t> for ImageBox {
            type Error = Infallible;

            fn include_pages(
                &mut self,
                includes: &[IncludeRef<'t>],
            ) -> Result<Vec<FetchedPage<'t>>, Infallible> {
                Ok(includes
                    .iter()
                    .map(|include| FetchedPage {
                        page_ref: include.page_ref().clone(),
                        content: Some(Cow::Borrowed(
                            "[[div style=\"float:{$float}; width:{$width}px; text-align:center; padding: 0.5rem 1.5rem 0.5rem 1.5rem; padding-{$float}: 0; background:white; clear:{$float};\"]]\n[[image {$image} class=\"[!--{$shadow}-]shadowed[!-- --]\" style=\"width:{$width}px; [!--{$showBorder}-]border: 1px solid black;[!-- --] margin: 0 0 5px 0;\"]]\n[!--{$caption}--]@@@@\n[[size 80%]]//{$caption}//[[/size]]\n[!-- --]\n[[/div]]",
                        )),
                    })
                    .collect())
            }

            fn no_such_include(
                &mut self,
                _: &PageRef,
            ) -> Result<Cow<'t, str>, Infallible> {
                unreachable!("fixture ImageBox exists")
            }
        }

        let settings = WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot);
        let source = "Before\n\n[[include ImageBox\n| image=fixture.png\n| width=500\n| float=right\n| showBorder=\n| caption=Caption\n]]\n\nAfter";
        let (expanded, _) =
            ftml::include(source, &settings, ImageBox, || panic!()).unwrap();
        let mut rendered_source = strip_comments(expanded);
        ftml::preprocess(&mut rendered_source);
        let tokens = ftml::tokenize(&rendered_source);
        let page_info = PageInfo {
            page: Cow::Borrowed("who-we-are"),
            category: None,
            site: Cow::Borrowed("test"),
            title: Cow::Borrowed("Fixture"),
            alt_title: None,
            score: ScoreValue::Integer(0),
            tags: vec![],
            language: Cow::Borrowed("default"),
        };
        let (tree, errors) = ftml::parse(&tokens, &page_info, &settings).into();
        assert!(errors.is_empty(), "{errors:?}");
        let html = HtmlRender.render(&tree, &page_info, &settings).body;
        let imagebox = html.split("<div style=\"float:right;").nth(1).unwrap();
        let children = imagebox
            .split_once('>')
            .unwrap()
            .1
            .split_once("</div>")
            .unwrap()
            .0;
        assert!(children.starts_with("<img "), "{html}");
        assert_eq!(children.matches("<br").count(), 1, "{html}");
        assert!(children.contains("<em>Caption</em>"), "{html}");
    }
}
