use crate::utils::get_slug;
use regex::{Captures, Regex};
use std::sync::LazyLock;

// Keep these patterns aligned with Wikidot's DependencyFixer::fixLinks.
static TRIPLE_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\[\[\[)([^\]\|]+?)((\s*\|[^\]]*?)?\]\]\])")
        .expect("valid legacy triple-link regex")
});
static INCLUDE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im-u)^\[\[include ([a-zA-Z0-9\s\-]+?)(?:\]\])$")
        .expect("valid legacy include regex")
});

/// Rewrite legacy dependency syntax in wikitext after a page slug changes.
pub fn rewrite_move_dependencies(source: &str, old_slug: &str, new_slug: &str) -> String {
    let links = TRIPLE_LINK.replace_all(source, |captures: &Captures<'_>| {
        let target = captures.get(2).unwrap().as_str();
        if get_slug(target) != old_slug {
            return captures[0].to_owned();
        }
        format!("[[[{}{}", new_slug, &captures[3])
    });
    INCLUDE
        .replace_all(&links, |captures: &Captures<'_>| {
            let target = captures.get(1).unwrap().as_str().trim();
            if get_slug(target) != old_slug {
                return captures[0].to_owned();
            }
            format!("[[include {new_slug}]]")
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::rewrite_move_dependencies;

    #[test]
    fn triple_links_normalize_targets_and_preserve_labels_and_closing_text() {
        let source = "[[[Old Page|Label]]] [[[ OLD   PAGE  | label with spaces ]]] [[[old-page]]] [[[another page|Old Page]]]";
        let expected = "[[[new-page|Label]]] [[[new-page  | label with spaces ]]] [[[new-page]]] [[[another page|Old Page]]]";
        assert_eq!(
            rewrite_move_dependencies(source, "old-page", "new-page"),
            expected
        );
    }

    #[test]
    fn triple_links_only_match_the_legacy_target_and_closing_pattern() {
        let source = "[[old-page]] [[[old-page|label]suffix]]] [[[old-page||two pipes]]] [[[old-page|label]]]";
        let expected = "[[old-page]] [[[old-page|label]suffix]]] [[[new-page||two pipes]]] [[[new-page|label]]]";
        assert_eq!(
            rewrite_move_dependencies(source, "old-page", "new-page"),
            expected
        );
    }

    #[test]
    fn includes_replace_full_case_insensitive_lines_only() {
        let source = "[[include Old Page]]\n[[INCLUDE OLD  PAGE]]\n[[include old-page]]\n[[include another page]]";
        let expected = "[[include new-page]]\n[[include new-page]]\n[[include new-page]]\n[[include another page]]";
        assert_eq!(
            rewrite_move_dependencies(source, "old-page", "new-page"),
            expected
        );
    }

    #[test]
    fn includes_do_not_rewrite_outside_the_legacy_character_class_or_line_boundary() {
        let source = "before [[include Old Page]]\n [[include Old Page]]\n[[include old_page]]\n[[include category:old-page]]\n[[include Old Page |arg=x]]\n[[include Old Page]] trailing\n[[include Old Page ]]";
        let expected = "before [[include Old Page]]\n [[include Old Page]]\n[[include old_page]]\n[[include category:old-page]]\n[[include Old Page |arg=x]]\n[[include Old Page]] trailing\n[[include new-page]]";
        assert_eq!(
            rewrite_move_dependencies(source, "old-page", "new-page"),
            expected
        );
    }

    #[test]
    fn links_can_rewrite_category_slugs_but_includes_cannot_match_colons() {
        let source = "[[[Category:Old Page|look]]] \n[[include Category:Old Page]]";
        let expected = "[[[category:new-page|look]]] \n[[include Category:Old Page]]";
        assert_eq!(
            rewrite_move_dependencies(source, "category:old-page", "category:new-page"),
            expected
        );
    }

    #[test]
    fn include_character_class_is_ascii_even_when_case_insensitive() {
        let source = "[[include OKD PAGE]]\n[[include OKD PAGE]]";
        let expected = "[[include OKD PAGE]]\n[[include new-page]]";
        assert_eq!(
            rewrite_move_dependencies(source, "okd-page", "new-page"),
            expected
        );
    }

    #[test]
    fn unrelated_or_already_updated_source_is_unchanged() {
        let source = "[[[new-page|Label]]] and [[include new-page]]\n[[[other-page]]]";
        assert_eq!(
            rewrite_move_dependencies(source, "old-page", "new-page"),
            source
        );
        assert_eq!(rewrite_move_dependencies("", "old-page", "new-page"), "");
    }
}
