/*
 * render/html/element/list.rs
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
use crate::tree::{AttributeMap, ListItem, ListType};

pub fn render_list(
    ctx: &mut HtmlContext,
    ltype: ListType,
    list_items: &[ListItem],
    attributes: &AttributeMap,
) {
    debug!(
        "Rendering list '{}' (items {})",
        ltype.name(),
        list_items.len(),
    );
    let list_tag = ltype.html_tag();
    let mut tag = ctx.html().tag(list_tag);

    tag.attr(attr!(;; attributes)).inner(|ctx| {
        let mut items = list_items.iter().peekable();
        while let Some(item) = items.next() {
            match item {
                ListItem::Elements {
                    elements,
                    attributes,
                } => {
                    ctx.html().li().attr(attr!(;; attributes)).inner(|ctx| {
                        render_elements(ctx, elements);
                        while let Some(ListItem::SubList { element }) = items.peek() {
                            render_element(ctx, element);
                            items.next();
                        }
                    });
                }
                ListItem::SubList { element } => render_element(ctx, element),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::data::PageInfo;
    use crate::layout::Layout;
    use crate::render::{Render, html::HtmlRender};
    use crate::settings::{WikitextMode, WikitextSettings};

    fn render(source: &str) -> String {
        let page_info = PageInfo::dummy();
        let settings = WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot);
        let mut text = source.to_owned();
        crate::preprocess(&mut text);
        let tokens = crate::tokenize(&text);
        let (tree, errors) = crate::parse(&tokens, &page_info, &settings).into();
        assert!(errors.is_empty(), "Unexpected parse errors: {errors:?}");
        HtmlRender.render(&tree, &page_info, &settings).body
    }

    #[test]
    fn nested_navigation_lists_belong_to_their_parent_items_across_groups() {
        let html = render(
            "* **Home**\n * [[[home:public|Join]]]\n * [[[home:help|Help]]]\n\n* **Profiles**\n * [[[roster|Characters]]]",
        );
        assert!(html.contains("<li><strong>Home</strong><ul><li>"), "{html}");
        assert!(
            html.contains("Help</a></li></ul></li><li><strong>Profiles</strong><ul><li>"),
            "{html}"
        );
        assert!(
            html.contains("Characters</a></li></ul></li></ul>"),
            "{html}"
        );
    }

    #[test]
    fn numbered_list_continues_after_nested_items() {
        let html = render("# One\n # Sub\n# Two");
        assert!(
            html.contains("<ol><li>One<ol><li>Sub</li></ol></li><li>Two</li></ol>"),
            "{html}"
        );
    }
}
