/*
 * parsing/rule/impls/block/blocks/image.rs
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
use crate::tree::{FileSource, FloatAlignment, LinkLocation};

pub const BLOCK_IMAGE: BlockRule = BlockRule {
    name: "block-image",
    accepts_names: &["image", "=image", "<image", ">image", "f<image", "f>image"],
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
    debug!("Parsing image block (name {name}, in-head {in_head})");
    assert!(!flag_star, "Image doesn't allow star flag");
    assert!(!flag_score, "Image doesn't allow score flag");
    assert_block_name(&BLOCK_IMAGE, name);

    let (source, mut arguments) =
        parser.get_head_name_lenient_map(&BLOCK_IMAGE, in_head)?;
    let link = arguments.get("link").map(LinkLocation::parse);
    let alignment = FloatAlignment::parse(name);

    // Parse the image source based on format
    let source = match FileSource::parse(source) {
        Some(source) => source,
        None => return Err(parser.make_err(ParseErrorKind::BlockMalformedArguments)),
    };

    // Build image
    let element = Element::Image {
        source,
        link,
        alignment,
        attributes: arguments.to_attribute_map(parser.settings()),
    };

    ok!(element)
}

#[cfg(test)]
mod test {
    use crate::data::PageInfo;
    use crate::layout::Layout;
    use crate::settings::{WikitextMode, WikitextSettings};
    use crate::tree::{Element, FileSource};

    fn first_image(text: &str) -> Element<'static> {
        let settings = WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot);
        let tokens = crate::tokenize(text);
        let page_info = PageInfo::dummy();
        let (tree, _) = crate::parse(&tokens, &page_info, &settings).into();
        let paragraph = match &tree.elements[0] {
            Element::Container(container) => container.elements().to_vec(),
            other => panic!("expected paragraph, got {other:?}"),
        };
        paragraph
            .into_iter()
            .find(|element| matches!(element, Element::Image { .. }))
            .expect("image element")
            .to_owned()
    }

    #[test]
    fn empty_source_takes_the_first_word_and_skips_unreadable_arguments_like_wikidot() {
        // An empty template variable leaves the style attribute first.
        let image = first_image(
            "[[image  style=\"margin: 0; border: 0;\" width=\"100%\" alt=\"\"]]",
        );
        let Element::Image {
            source, attributes, ..
        } = image
        else {
            unreachable!();
        };
        assert_eq!(
            source,
            FileSource::File1 {
                file: "style=\"margin:".into()
            }
        );
        let attributes = attributes.get();
        assert_eq!(attributes.get("width").map(AsRef::as_ref), Some("100%"));
        assert_eq!(attributes.get("alt").map(AsRef::as_ref), Some(""));
    }
}
