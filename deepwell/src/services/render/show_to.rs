//! Wikidot `:snippets:suo` "show to" regions, which reveal content to listed users.
//!
//! Wikidot sends nothing to anonymous visitors (the snippet wraps the region in a
//! `ListUsers users="."` module) and hides it with CSS from signed-in users it
//! does not list. Compiled HTML here is shared by every reader, so it omits every
//! region; a signed-in viewer listed by a region gets that fragment rendered for
//! them at view time with the region's content kept, and nobody else receives it.

const BEGIN: &str = "[[include :snippets:suo begin code";
const END: &str = "[[include :snippets:suo end code]]";

/// A `type=showto` region: `start..end` spans the markers, `body` their content.
/// Offsets index both the source and its ASCII-lowercase copy.
struct Region {
    start: usize,
    body: std::ops::Range<usize>,
    end: usize,
    lists_viewer: bool,
}

/// Every `type=showto` region. A region without an end marker runs to the end of
/// the source. `viewer` is a user slug; `None` (anonymous) is listed by no region.
fn show_to_regions(lower: &str, viewer: Option<&str>) -> Vec<Region> {
    let mut regions = Vec::new();
    let mut search = 0;
    while let Some(found) = lower[search..].find(BEGIN) {
        let start = search + found;
        let Some(header_length) = lower[start..].find("]]") else {
            break;
        };
        let header = &lower[start..start + header_length];
        search = start + header_length + "]]".len();
        if !is_show_to(header) {
            continue;
        }
        let (body_end, end) = match lower[search..].find(END) {
            Some(offset) => (search + offset, search + offset + END.len()),
            None => (lower.len(), lower.len()),
        };
        regions.push(Region {
            start,
            body: search..body_end,
            end,
            lists_viewer: viewer.is_some_and(|viewer| lists_user(header, viewer)),
        });
        search = end;
    }
    regions
}

/// Remove every `type=showto` region with its delimiters. A region without an end
/// marker hides the rest of the source; other region types are left unchanged.
pub(super) fn strip_show_to_regions(source: String) -> String {
    let regions = show_to_regions(&source.to_ascii_lowercase(), None);
    if regions.is_empty() {
        return source;
    }
    let mut output = String::with_capacity(source.len());
    let mut copied = 0;
    for region in regions {
        output.push_str(&source[copied..region.start]);
        copied = region.end;
    }
    output.push_str(&source[copied..]);
    output
}

/// Unwrap the regions that list `viewer` (a user slug), keeping their content
/// without the markers; other regions stay for `strip_show_to_regions`. `None`
/// when no region lists the viewer, so the shared compiled HTML already applies.
pub(super) fn reveal_show_to_regions(source: &str, viewer: &str) -> Option<String> {
    let viewer = viewer.to_ascii_lowercase();
    let regions = show_to_regions(&source.to_ascii_lowercase(), Some(&viewer));
    if !regions.iter().any(|region| region.lists_viewer) {
        return None;
    }
    let mut output = String::with_capacity(source.len());
    let mut copied = 0;
    for region in regions.into_iter().filter(|region| region.lists_viewer) {
        output.push_str(&source[copied..region.start]);
        output.push_str(&source[region.body]);
        copied = region.end;
    }
    output.push_str(&source[copied..]);
    Some(output)
}

fn header_arguments(header: &str) -> impl Iterator<Item = (&str, &str)> {
    header.split('|').filter_map(|argument| {
        argument
            .split_once('=')
            .map(|(key, value)| (key.trim(), value.trim()))
    })
}

fn is_show_to(header: &str) -> bool {
    header_arguments(header).any(|argument| argument == ("type", "showto"))
}

/// The snippet takes the listed users as `user`, `user0` ... `user99`.
fn lists_user(header: &str, viewer: &str) -> bool {
    header_arguments(header).any(|(key, value)| {
        let number = key.strip_prefix("user");
        number.is_some_and(|number| number.bytes().all(|byte| byte.is_ascii_digit()))
            && !value.is_empty()
            && value == viewer
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAV: &str = "* Home\n[[include :snippets:suo BEGIN CODE |type=showto | user1=ozmaasimov | user2=Allicat ]]\n* **Admin**\n * [[[_admin|Site Manager]]]\n[[include :snippets:suo END CODE]]\n* Profiles";

    #[test]
    fn removes_show_to_regions_with_their_markers() {
        assert_eq!(strip_show_to_regions(NAV.into()), "* Home\n\n* Profiles");
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
        assert_eq!(reveal_show_to_regions(source, "a"), None);
    }

    #[test]
    fn listed_viewers_get_the_region_content_without_markers() {
        let revealed =
            "* Home\n\n* **Admin**\n * [[[_admin|Site Manager]]]\n\n* Profiles";
        assert_eq!(
            reveal_show_to_regions(NAV, "ozmaasimov").as_deref(),
            Some(revealed),
        );
        assert_eq!(
            reveal_show_to_regions(NAV, "allicat").as_deref(),
            Some(revealed),
        );
    }

    #[test]
    fn unlisted_viewers_and_partial_names_get_nothing() {
        for viewer in ["luridel", "ozma", "ozmaasimov2", ""] {
            assert_eq!(reveal_show_to_regions(NAV, viewer), None, "{viewer}");
        }
    }

    #[test]
    fn only_the_regions_listing_the_viewer_are_revealed() {
        let source = "[[include :snippets:suo BEGIN CODE |type=showto |user1=a]]\nFor A\n[[include :snippets:suo END CODE]]\n[[include :snippets:suo BEGIN CODE |type=showto |user=b]]\nFor B\n[[include :snippets:suo END CODE]]";
        let revealed = reveal_show_to_regions(source, "b").unwrap();
        assert_eq!(strip_show_to_regions(revealed), "\n\nFor B\n");
    }
}
