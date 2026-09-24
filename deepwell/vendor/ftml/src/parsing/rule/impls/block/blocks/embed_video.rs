/*
 * parsing/rule/impls/block/blocks/embed_video.rs
 *
 * ftml - Library to parse Wikidot text
 * Copyright (C) 2019-2026 Wikijump Team
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */

//! Wikidot's `[[embedvideo]]<iframe ...></iframe>[[/embedvideo]]`.
//!
//! Wikidot outputs the pasted HTML as is. Here only a single `<iframe>` from an
//! allow-listed video host is accepted, and it keeps only safe attributes.
//! Anything else fails the block, so its source is shown as escaped text.

use super::prelude::*;
use crate::tree::AttributeMap;
use regex::Regex;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::LazyLock;

static IFRAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)^\s*<iframe\b([^>]*)>\s*</iframe>\s*$").unwrap());

static IFRAME_ATTRIBUTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"([A-Za-z-]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+)))?"#)
        .unwrap()
});

const VIDEO_HOSTS: [&str; 5] = [
    "youtube.com",
    "www.youtube.com",
    "youtube-nocookie.com",
    "www.youtube-nocookie.com",
    "player.vimeo.com",
];

const SAFE_IFRAME_ATTRIBUTES: [&str; 6] = [
    "src",
    "width",
    "height",
    "frameborder",
    "allow",
    "allowfullscreen",
];

pub const BLOCK_EMBED_VIDEO: BlockRule = BlockRule {
    name: "block-embedvideo",
    accepts_names: &["embedvideo"],
    accepts_star: false,
    accepts_score: false,
    accepts_newlines: true,
    parse_fn,
};

fn parse_fn<'r, 't>(
    parser: &mut Parser<'r, 't>,
    name: &'t str,
    flag_star: bool,
    flag_score: bool,
    in_head: bool,
) -> ParseResult<'r, 't, Elements<'t>> {
    debug!("Parsing embedvideo block (in-head {in_head})");
    assert!(!flag_star, "Embedvideo doesn't allow star flag");
    assert!(!flag_score, "Embedvideo doesn't allow score flag");
    assert_block_name(&BLOCK_EMBED_VIDEO, name);

    parser.get_head_none(&BLOCK_EMBED_VIDEO, in_head)?;
    let body = parser.get_body_text(&BLOCK_EMBED_VIDEO)?;
    let attributes = parse_video_iframe(body)
        .ok_or_else(|| parser.make_err(ParseErrorKind::BlockMalformedArguments))?;

    ok!(Element::EmbedVideo { attributes })
}

/// Returns the safe attributes of a lone `<iframe>` whose `src` is on a video host.
fn parse_video_iframe(body: &str) -> Option<AttributeMap<'_>> {
    let attributes = IFRAME.captures(body)?.get(1)?.as_str();
    let mut safe = BTreeMap::new();

    for captures in IFRAME_ATTRIBUTE.captures_iter(attributes) {
        let key = captures[1].to_ascii_lowercase();
        if !SAFE_IFRAME_ATTRIBUTES.contains(&key.as_str()) {
            continue;
        }

        // allowfullscreen is boolean: its value is irrelevant.
        let value = match key.as_str() {
            "allowfullscreen" => "",
            _ => (2..=4)
                .find_map(|group| captures.get(group))
                .map_or("", |value| value.as_str()),
        };

        safe.insert(Cow::Owned(key), Cow::Borrowed(value));
    }

    if !is_video_url(safe.get("src")?) {
        warn!("Embedvideo iframe is not from an allowed video host");
        return None;
    }

    Some(AttributeMap::from(safe))
}

fn is_video_url(src: &str) -> bool {
    let Some(rest) = src.strip_prefix("https://") else {
        return false;
    };

    // A port or userinfo leaves the host unmatched, so it is rejected too.
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    VIDEO_HOSTS
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(host))
}

#[cfg(test)]
mod test {
    use crate::data::PageInfo;
    use crate::layout::Layout;
    use crate::render::{Render, html::HtmlRender};
    use crate::settings::{WikitextMode, WikitextSettings};

    fn render(text: &str) -> String {
        let settings = WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot);
        let tokens = crate::tokenize(text);
        let page_info = PageInfo::dummy();
        let (tree, _) = crate::parse(&tokens, &page_info, &settings).into();
        HtmlRender.render(&tree, &page_info, &settings).body
    }

    #[test]
    fn youtube_iframe_keeps_only_safe_attributes() {
        // writing:2024-04-21-the-lich-king-falls-the-lich-king-rises
        let html = render(
            "[[embedvideo]]\n<iframe width=\"560\" height=\"315\" src=\"https://www.youtube.com/embed/8MUc-rQD8j0?si=2XggSzXhl1qRTkdJ\" title=\"YouTube video player\" frameborder=\"0\" allow=\"accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share\" referrerpolicy=\"strict-origin-when-cross-origin\" allowfullscreen></iframe>\n[[/embedvideo]]",
        );
        assert_eq!(
            html,
            "<p><iframe allow=\"accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share\" allowfullscreen frameborder=\"0\" height=\"315\" src=\"https://www.youtube.com/embed/8MUc-rQD8j0?si=2XggSzXhl1qRTkdJ\" width=\"560\"></iframe></p>",
        );
    }

    #[test]
    fn event_handlers_are_dropped_from_allowed_hosts() {
        assert_eq!(
            render(
                "[[embedvideo]]\n<iframe src='https://player.vimeo.com/video/1' onload=\"alert(1)\" style=\"x\"></iframe>\n[[/embedvideo]]",
            ),
            "<p><iframe src=\"https://player.vimeo.com/video/1\"></iframe></p>",
        );
    }

    #[test]
    fn iframe_from_other_host_is_escaped_text() {
        let html = render(
            "[[embedvideo]]\n<iframe src=\"https://evil.example/youtube.com/embed/x\" onload=\"alert(1)\"></iframe>\n[[/embedvideo]]",
        );
        // The host check reads the real host, not a "youtube.com" path segment.
        assert_eq!(
            html,
            "<p>[[embedvideo]]<br>&lt;iframe src=&quot;<a href=\"https://evil.example/youtube.com/embed/x\">https://evil.example/youtube.com/embed/x</a>&quot; onload=&quot;alert(1)&quot;&gt;&lt;/iframe&gt;<br>[[/embedvideo]]</p>",
        );
    }

    #[test]
    fn non_iframe_body_is_rejected() {
        let html = render(
            "[[embedvideo]]\n<script src=\"https://www.youtube.com/x.js\"></script>\n[[/embedvideo]]",
        );
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("<iframe"), "{html}");
    }
}
