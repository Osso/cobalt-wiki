/*
 * parsing/rule/impls/blockquote.rs
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

use super::prelude::*;
use crate::layout::Layout;
use crate::parsing::paragraph::ParagraphStack;
use crate::parsing::{DepthItem, DepthList, process_depths};
use crate::tree::{AttributeMap, Container, ContainerType};

const MAX_BLOCKQUOTE_DEPTH: usize = 30;

pub const RULE_BLOCKQUOTE: Rule = Rule {
    name: "blockquote",
    position: LineRequirement::StartOfLine,
    try_consume_fn,
};

fn try_consume_fn<'r, 't>(
    parser: &mut Parser<'r, 't>,
) -> ParseResult<'r, 't, Elements<'t>> {
    debug!("Parsing nested native blockquotes");

    // Context variables
    let mut depths = Vec::new();
    let mut errors = Vec::new();
    let mut dropped = false;

    // Produce a depth list with elements
    loop {
        let current = parser.current();
        let depth = match current.token {
            // 1 or more ">"s in one token. Return ASCII length.
            Token::Quote => current.slice.len(),

            // Invalid token, bail
            _ => {
                warn!("Didn't find blockquote token, ending list iteration");
                break;
            }
        };
        parser.step()?;

        // Wikidot's blockquote rule takes every ">" line but keeps only those
        // with a space after the ">"s, so ">text" lines vanish. Sites use it
        // to hide text ("with no space between the > and the text").
        if parser.settings().layout == Layout::Wikidot
            && parser.current().token != Token::Whitespace
        {
            skip_line(parser)?;
            dropped = true;
            continue;
        }

        parser.get_optional_space()?; // allow whitespace after ">"

        // Check that the depth isn't obscenely deep, to avoid DOS attacks via stack overflow.
        if depth > MAX_BLOCKQUOTE_DEPTH {
            debug!(
                "Native blockquote has a depth ({depth}) greater than the maximum ({MAX_BLOCKQUOTE_DEPTH})! Failing"
            );
            return Err(parser.make_err(ParseErrorKind::BlockquoteDepthExceeded));
        }

        // Parse elements until we hit the end of the line
        let mut paragraph_safe = true;
        let mut elements = collect_consume(
            parser,
            RULE_BLOCKQUOTE,
            &[
                ParseCondition::current(Token::LineBreak),
                ParseCondition::current(Token::ParagraphBreak),
                ParseCondition::current(Token::InputEnd),
            ],
            &[],
            None,
        )?
        .chain(&mut errors, &mut paragraph_safe);

        // Add a line break for the end of the line
        elements.push(Element::LineBreak);

        // Append blockquote line
        //
        // Depth lists expect zero-based list depths, but tokens are one-based.
        // So, we subtract one.
        //
        // This will not overflow because Token::Quote requires at least one ">".
        depths.push((depth - 1, (), (elements, paragraph_safe)))
    }

    // This blockquote has no rows, so the rule fails,
    // unless its lines were dropped as Wikidot does.
    if depths.is_empty() {
        if dropped {
            return ok!(false; Vec::new(), errors);
        }
        return Err(parser.make_err(ParseErrorKind::RuleFailed));
    }

    let depth_lists = process_depths((), depths);
    let elements: Vec<Element> = depth_lists
        .into_iter()
        .map(|(_, depth_list)| build_blockquote_element(depth_list))
        .collect();

    ok!(false; elements, errors)
}

/// Consume the rest of the line, including its line or paragraph break.
fn skip_line(parser: &mut Parser) -> Result<(), ParseError> {
    loop {
        match parser.current().token {
            Token::InputEnd => return Ok(()),
            Token::LineBreak | Token::ParagraphBreak => return parser.step().map(|_| ()),
            _ => {
                parser.step()?;
            }
        }
    }
}

fn build_blockquote_element(list: DepthList<(), (Vec<Element>, bool)>) -> Element {
    let mut stack = ParagraphStack::new();

    // Convert depth list into a list of elements
    for item in list {
        match item {
            DepthItem::Item((elements, paragraph_safe)) => {
                for element in elements {
                    stack.push_element(element, paragraph_safe);
                }
            }
            DepthItem::List(_, list) => {
                let blockquote = build_blockquote_element(list);
                stack.pop_line_break();
                stack.push_element(blockquote, false);
            }
        }
    }

    stack.pop_line_break();

    Element::Container(Container::new(
        ContainerType::Blockquote,
        stack.into_elements(),
        AttributeMap::new(),
    ))
}

#[cfg(test)]
mod tests {
    use crate::data::PageInfo;
    use crate::layout::Layout;
    use crate::render::{Render, html::HtmlRender};
    use crate::settings::{WikitextMode, WikitextSettings};

    fn render(source: &str, layout: Layout) -> String {
        let page_info = PageInfo::dummy();
        let settings = WikitextSettings::from_mode(WikitextMode::Page, layout);
        let mut text = source.to_owned();
        crate::preprocess(&mut text);
        let tokens = crate::tokenize(&text);
        let (tree, errors) = crate::parse(&tokens, &page_info, &settings).into();
        assert!(errors.is_empty(), "{errors:?}");
        HtmlRender.render(&tree, &page_info, &settings).body
    }

    #[test]
    fn wikidot_drops_quote_lines_without_a_space() {
        // new-profile: the whole quoted block disappears on Wikidot.
        assert_eq!(
            render(
                ">[[div style=\"float:right;\"]]\n>[[toc]]\n>[[/div]]\n\nWe know",
                Layout::Wikidot,
            ),
            "<p>We know</p>",
        );

        // recruitment-status: ">" hides the message that is not current.
        assert_eq!(
            render(">Recruiting is open!\n\nWe're closed.", Layout::Wikidot),
            "<p>We&#39;re closed.</p>",
        );

        // Only the lines with a space stay in the quote.
        let mixed = render("> kept\n>hidden\n> also", Layout::Wikidot);
        assert!(
            mixed.starts_with("<blockquote>")
                && mixed.contains("kept")
                && mixed.contains("also")
                && !mixed.contains("hidden"),
            "{mixed}"
        );
    }

    #[test]
    fn wikijump_keeps_quote_lines_without_a_space() {
        let native = render(">text", Layout::Wikijump);
        assert!(
            native.contains("<blockquote") && native.contains("text"),
            "{native}"
        );
    }
}
