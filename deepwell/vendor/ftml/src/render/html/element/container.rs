/*
 * render/html/element/container.rs
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
use crate::tree::{Container, ContainerType, HtmlTag};

pub fn render_container(ctx: &mut HtmlContext, container: &Container) {
    debug!("Rendering container '{}'", container.ctype().name());

    // Wikidot does not create a paragraph when its first token is an image.
    if ctx.layout() == Layout::Wikidot
        && container.ctype() == ContainerType::Paragraph
        && matches!(container.elements().first(), Some(Element::Image { .. }))
    {
        render_elements(ctx, container.elements());
        return;
    }

    match container.ctype() {
        // We wrap with <rp> around the <rt> contents
        ContainerType::RubyText => {
            ctx.html().rp().contents("(");
            render_container_internal(ctx, container);
            ctx.html().rp().contents(")");
        }

        // Render normally
        _ => render_container_internal(ctx, container),
    }
}

pub fn render_container_internal(ctx: &mut HtmlContext, container: &Container) {
    // Get HTML tag type for this type of container
    let layout = ctx.layout();
    let tag_spec = container.ctype().html_tag(layout, ctx);

    // Get correct ID, based on the render setting
    let random_id = choose_id(ctx, &tag_spec);

    // Build the tag
    let mut tag = ctx.html().tag(tag_spec.tag());

    // Merge the class attribute with the container's class, if it conflicts
    match tag_spec {
        HtmlTag::Tag(_) => tag.attr(attr!(;; container.attributes())),
        HtmlTag::TagAndClass { class, .. } => tag.attr(attr!(
            "class" => class;;
            container.attributes(),
        )),
        HtmlTag::TagAndStyle { style, .. } => tag.attr(attr!(
            "style" => style;;
            container.attributes(),
        )),
        HtmlTag::TagAndId { id, .. } => tag.attr(attr!(
            "id" => match random_id {
                Some(ref id) => id,
                None => &id,
            };;
            container.attributes(),
        )),
    };

    // Add container internals
    tag.contents(container.elements());
}

pub fn render_color(ctx: &mut HtmlContext, color: &str, elements: &[Element]) {
    debug!("Rendering color container (color '{color}')");

    ctx.html()
        .span()
        .attr(attr!(
            "style" => "color: " color ";",
        ))
        .contents(elements);
}

fn choose_id(ctx: &mut HtmlContext, tag_spec: &HtmlTag) -> Option<String> {
    // If we're in a situation where we want a randomly generated ID
    if matches!(tag_spec, HtmlTag::TagAndId { .. }) && !ctx.settings().use_true_ids {
        Some(ctx.random().generate_html_id())
    } else {
        None
    }
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
        assert!(errors.is_empty(), "{errors:?}");
        HtmlRender.render(&tree, &page_info, &settings).body
    }

    #[test]
    fn wikidot_leading_image_is_not_wrapped_but_inline_image_and_prose_are() {
        let leading = render("[[div]]\n[[image fixture.png]]\nCaption\n[[/div]]");
        assert!(
            leading.starts_with("<div><img ") && leading.contains("<br>Caption</div>"),
            "{leading}"
        );

        let inline = render("[[div]]\nText [[image fixture.png]] Caption\n[[/div]]");
        assert!(
            inline.starts_with("<div><p>Text <img ")
                && inline.ends_with(" Caption</p></div>"),
            "{inline}"
        );

        assert_eq!(
            render("[[div]]\nProse\n[[/div]]"),
            "<div><p>Prose</p></div>"
        );
    }
}
