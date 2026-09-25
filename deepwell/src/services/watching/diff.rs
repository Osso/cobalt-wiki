//! Bounded, change-only rendered-text summaries for watcher emails.

use similar::{ChangeTag, TextDiff};

const MAX_SUMMARY_CHARS: usize = 1_000;

/// Summarizes changed rendered text, including markers in the character limit.
pub fn rendered_diff(before: Option<&str>, after: &str) -> String {
    let mut summary = String::new();
    let Some(before) = before else {
        append_change(&mut summary, ChangeTag::Insert, after);
        return summary;
    };

    let diff = TextDiff::configure().diff_words(before, after);
    let mut pending_tag = None;
    let mut pending_text = String::new();

    for change in diff.iter_all_changes() {
        let tag = change.tag();
        if pending_tag != Some(tag) {
            if let Some(previous_tag) = pending_tag {
                if append_change(&mut summary, previous_tag, &pending_text) {
                    return summary;
                }
                pending_text.clear();
            }
            pending_tag = Some(tag);
        }
        if tag == ChangeTag::Equal {
            pending_tag = None;
        } else {
            pending_text.push_str(change.value());
        }
    }

    if let Some(tag) = pending_tag {
        append_change(&mut summary, tag, &pending_text);
    }
    summary
}

/// Returns whether the summary has reached its character budget.
fn append_change(summary: &mut String, tag: ChangeTag, text: &str) -> bool {
    if tag == ChangeTag::Equal || text.is_empty() {
        return false;
    }

    let marker = match tag {
        ChangeTag::Delete => "- ",
        ChangeTag::Insert => "+ ",
        ChangeTag::Equal => unreachable!(),
    };
    let separator = usize::from(!summary.is_empty());
    let available = MAX_SUMMARY_CHARS - summary.chars().count();
    if available <= separator + marker.chars().count() {
        return true;
    }
    if separator != 0 {
        summary.push('\n');
    }
    summary.push_str(marker);
    summary.extend(
        text.chars()
            .take(available - separator - marker.chars().count()),
    );
    summary.chars().count() == MAX_SUMMARY_CHARS
}

#[cfg(test)]
mod tests {
    use super::rendered_diff;

    #[test]
    fn reports_separated_edits_without_unchanged_text() {
        assert_eq!(
            rendered_diff(Some("red one blue two green"), "red ONE blue TWO green"),
            "- one\n+ ONE\n- two\n+ TWO"
        );
    }

    #[test]
    fn reports_additions_and_removals() {
        assert_eq!(rendered_diff(Some("old"), "new"), "- old\n+ new");
        assert_eq!(rendered_diff(Some("old"), ""), "- old");
        assert_eq!(rendered_diff(Some(""), "new"), "+ new");
    }

    #[test]
    fn unchanged_and_empty_inputs_have_no_changes() {
        assert_eq!(rendered_diff(Some("same"), "same"), "");
        assert_eq!(rendered_diff(Some(""), ""), "");
        assert_eq!(rendered_diff(None, ""), "");
    }

    #[test]
    fn creation_includes_only_added_content() {
        assert_eq!(rendered_diff(None, "fresh page"), "+ fresh page");
    }

    #[test]
    fn limits_creation_to_one_thousand_unicode_characters_including_marker() {
        let after = "界".repeat(1_100);
        let summary = rendered_diff(None, &after);
        assert_eq!(summary.chars().count(), 1_000);
        assert_eq!(summary, format!("+ {}", "界".repeat(998)));
    }

    #[test]
    fn limits_combined_removals_and_additions_to_one_thousand_characters() {
        let before = "界".repeat(600);
        let after = "é".repeat(600);
        let summary = rendered_diff(Some(&before), &after);
        assert_eq!(summary.chars().count(), 1_000);
        assert_eq!(summary, format!("- {}\n+ {}", before, "é".repeat(395)));
    }
}
