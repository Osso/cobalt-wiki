//! Wikidot `:snippets:suo` "show to" regions, which reveal content to listed users.
//!
//! Compiled HTML is shared by every reader, so a region restricted to named users is
//! omitted entirely, as anonymous Wikidot visitors saw it.

const BEGIN: &str = "[[include :snippets:suo begin code";
const END: &str = "[[include :snippets:suo end code]]";

/// Remove every `type=showto` region with its delimiters. A region without an end
/// marker hides the rest of the source; other region types are left unchanged.
pub(super) fn strip_show_to_regions(source: String) -> String {
    let lower = source.to_ascii_lowercase();
    let mut output = String::with_capacity(source.len());
    let mut copied = 0;
    let mut search = 0;
    while let Some(found) = lower[search..].find(BEGIN) {
        let start = search + found;
        let Some(header_length) = lower[start..].find("]]") else {
            break;
        };
        let header = &lower[start..start + header_length];
        search = start + header_length;
        if !is_show_to(header) {
            continue;
        }
        output.push_str(&source[copied..start]);
        copied = match lower[search..].find(END) {
            Some(end) => search + end + END.len(),
            None => source.len(),
        };
        search = copied;
    }
    output.push_str(&source[copied..]);
    output
}

fn is_show_to(header: &str) -> bool {
    header.split('|').any(|argument| {
        argument
            .split_once('=')
            .is_some_and(|(key, value)| key.trim() == "type" && value.trim() == "showto")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_show_to_regions_with_their_markers() {
        let source = "* Home\n[[include :snippets:suo BEGIN CODE |type=showto | user1=ozmaasimov ]]\n* **Admin**\n * [[[_admin|Site Manager]]]\n[[include :snippets:suo END CODE]]\n* Profiles";
        assert_eq!(strip_show_to_regions(source.into()), "* Home\n\n* Profiles");
    }

    #[test]
    fn an_unterminated_region_hides_the_rest_of_the_source() {
        let source =
            "Public\n[[include :snippets:suo BEGIN CODE |type=showto |user1=a]]\nPrivate";
        assert_eq!(strip_show_to_regions(source.into()), "Public\n");
    }

    #[test]
    fn other_region_types_and_ordinary_text_are_unchanged() {
        let source = "[[include :snippets:suo BEGIN CODE |type=hideto |user1=a]]\nShown\n[[include :snippets:suo END CODE]]\n[[include other]]";
        assert_eq!(strip_show_to_regions(source.into()), source);
    }
}
