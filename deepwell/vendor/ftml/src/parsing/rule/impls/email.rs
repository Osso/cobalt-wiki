/*
 * parsing/rule/impls/email.rs
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

pub const RULE_EMAIL: Rule = Rule {
    name: "email",
    position: LineRequirement::Any,
    try_consume_fn,
};

fn try_consume_fn<'r, 't>(
    parser: &mut Parser<'r, 't>,
) -> ParseResult<'r, 't, Elements<'t>> {
    debug!("Consuming token as an email");
    ok!(Element::Email(cow!(parser.current().slice)))
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
    fn raw_span_after_block_is_not_an_email() {
        // bgc:pennings: form_data values arrive as @<...>@ raw spans in cells.
        let html = render(
            "[[table]]\n[[row]]\n[[cell]]@<J.C. Pennings>@[[/cell]]\n[[/row]]\n[[/table]]",
        );
        assert!(!html.contains("[[cell]]"), "{html}");
        assert!(!html.contains("mailto:"), "{html}");
        assert!(html.starts_with("<table>"), "{html}");
        assert!(html.contains(">J.C. Pennings</span></td>"), "{html}");
    }

    #[test]
    fn real_address_still_links() {
        let html = render("Contact: j.c.pennings+cobalt@mail.example.org now");
        assert!(
            html.contains(
                "<a href=\"mailto:j.c.pennings+cobalt@mail.example.org\">j.c.pennings+cobalt@mail.example.org</a>"
            ),
            "{html}"
        );
    }
}
