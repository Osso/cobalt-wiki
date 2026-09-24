/*
 * parsing/rule/impls/block/blocks/button.rs
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

//! Wikidot's `[[button type ...]]`, e.g. `[[button tags text="Click"]]` or
//! `[[button set-tags +_completed -@@ text="Publish"]]`.

use super::super::parser::LENIENT_ARGUMENT;
use super::prelude::*;
use std::borrow::Cow;

pub const BLOCK_BUTTON: BlockRule = BlockRule {
    name: "block-button",
    accepts_names: &["button"],
    accepts_star: false,
    accepts_score: false,
    accepts_newlines: false,
    parse_fn,
};

fn parse_fn<'r, 't>(
    parser: &mut Parser<'r, 't>,
    name: &'t str,
    flag_star: bool,
    flag_score: bool,
    in_head: bool,
) -> ParseResult<'r, 't, Elements<'t>> {
    debug!("Parsing button block (in-head {in_head})");
    assert!(!flag_star, "Button doesn't allow star flag");
    assert!(!flag_score, "Button doesn't allow score flag");
    assert_block_name(&BLOCK_BUTTON, name);

    let element = parser.get_head_value(&BLOCK_BUTTON, in_head, |parser, head| {
        head.and_then(parse_button)
            .ok_or_else(|| parser.make_err(ParseErrorKind::BlockMalformedArguments))
    })?;

    ok!(element)
}

/// Splits the head into Wikidot's button type, its `text` label, and the
/// remaining words (the tag changes of `set-tags`).
fn parse_button(head: &str) -> Option<Element<'_>> {
    let head = head.trim();
    let (button_type, rest) = head.split_once(char::is_whitespace).unwrap_or((head, ""));

    // Wikidot matches the type case-insensitively and reads '_' as '-'.
    let button_type = button_type.to_ascii_lowercase().replace('_', "-");
    let fallback = match button_type.as_str() {
        // Seen on wikidot.com, absent from its 2009 source: no known default label.
        "set-tags" => None,
        other => Some(default_text(other)?),
    };
    let text = LENIENT_ARGUMENT
        .captures_iter(rest)
        .find(|captures| captures[1].eq_ignore_ascii_case("text"))
        .and_then(|captures| captures.get(2))
        .map(|text| cow!(text.as_str()))
        .or(fallback.map(Cow::Borrowed))?;

    let tags = LENIENT_ARGUMENT
        .replace_all(rest, "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    Some(Element::Button {
        button_type: Cow::Owned(button_type),
        text,
        tags: Cow::Owned(tags),
    })
}

/// Wikidot's 2009 button types and their labels when `text` is omitted.
fn default_text(button_type: &str) -> Option<&'static str> {
    match button_type {
        "edit" => Some("edit"),
        "edit-append" => Some("append"),
        "edit-sections" => Some("edit sections"),
        "history" => Some("history"),
        "print" => Some("print"),
        "files" => Some("files"),
        "tags" => Some("tags"),
        "source" => Some("view source"),
        "talk" => Some("talk"),
        "backlinks" => Some("backlinks"),
        _ => None,
    }
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
    fn tags_button_renders_wikidot_markup_without_inline_js() {
        // writing:_template, step 2.
        assert_eq!(
            render("[[button tags text=\"Click here to open the Tags editor\"]]"),
            "<p><a class=\"wiki-standalone-button\" data-button-type=\"tags\">Click here to open the Tags editor</a></p>",
        );
    }

    #[test]
    fn set_tags_button_keeps_the_tag_changes() {
        // writing:_template: two spaces before text, and "@@" is not raw text.
        assert_eq!(
            render("[[button set-tags +_completed -@@  text=\"Publish\"]]"),
            "<p><a class=\"wiki-standalone-button\" data-button-type=\"set-tags\" data-tags=\"+_completed -@@\">Publish</a></p>",
        );
    }

    #[test]
    fn omitted_text_uses_wikidot_default_label() {
        assert_eq!(
            render("[[button edit_sections]]"),
            "<p><a class=\"wiki-standalone-button\" data-button-type=\"edit-sections\">edit sections</a></p>",
        );
    }

    #[test]
    fn unknown_button_type_is_not_a_button() {
        let html = render("[[button launch text=\"<b>x</b>\"]]");
        assert!(!html.contains("<a "), "{html}");
        assert!(!html.contains("<b>"), "{html}");
    }
}
