/*
 * parsing/rule/impls/block/blocks/gallery.rs
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
use std::borrow::Cow;

pub const BLOCK_GALLERY: BlockRule = BlockRule {
    name: "block-gallery",
    accepts_names: &["gallery"],
    accepts_star: false,
    accepts_score: false,
    accepts_newlines: true,
    parse_fn,
};

/// Wikidot's gallery sizes; anything else is "thumbnail".
const SIZES: [&str; 5] = ["small", "medium", "thumbnail", "square", "original"];

fn parse_fn<'r, 't>(
    parser: &mut Parser<'r, 't>,
    name: &'t str,
    flag_star: bool,
    flag_score: bool,
    in_head: bool,
) -> ParseResult<'r, 't, Elements<'t>> {
    debug!("Parsing gallery block (in-head {in_head})");
    assert!(!flag_star, "Gallery doesn't allow star flag");
    assert!(!flag_score, "Gallery doesn't allow score flag");
    assert_block_name(&BLOCK_GALLERY, name);

    let mut arguments = parser.get_head_lenient_map(&BLOCK_GALLERY, in_head)?;
    let size = arguments.get("size");
    let size = SIZES
        .into_iter()
        .find(|known| size.as_deref() == Some(*known))
        .unwrap_or("thumbnail");

    // Wikidot's list form is `: file` lines closed by [[/gallery]]. Anything
    // else leaves a bare [[gallery]] of the page's attachments.
    let mut body_parser = parser.clone();
    let sources = match body_parser.get_body_text(&BLOCK_GALLERY) {
        Ok(body) => parse_sources(body),
        Err(_) => None,
    };
    if sources.is_some() {
        parser.update(&body_parser);
    }

    let element = Element::Gallery {
        size: Cow::Borrowed(size),
        sources,
    };
    ok!(false; element)
}

/// The file names of `: file [attributes]` lines, or `None` unless every
/// line is one.
fn parse_sources(body: &str) -> Option<Vec<Cow<'_, str>>> {
    let rows = body
        .split('\n')
        .map(|line| line.strip_prefix(": ").filter(|row| !row.is_empty()))
        .collect::<Option<Vec<_>>>()?;

    Some(
        rows.into_iter()
            .filter_map(|row| row.split_whitespace().next())
            .map(Cow::Borrowed)
            .collect(),
    )
}
