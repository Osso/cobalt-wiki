/*
 * render/html/element/gallery.rs
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
use crate::tree::FileSource;
use std::borrow::Cow;

/// Wikidot's gallery markup, without its lightbox script. A `: file` list
/// adds Wikidot's lightbox and size classes; a bare gallery shows the page's
/// image attachments.
pub fn render_gallery(
    ctx: &mut HtmlContext,
    size: &str,
    sources: &Option<Vec<Cow<str>>>,
) {
    debug!(
        "Rendering gallery (size {size}, listed {})",
        sources.is_some()
    );

    let page = match &ctx.info().category {
        Some(category) => format!("{category}:{}", ctx.info().page),
        None => ctx.info().page.to_string(),
    };
    ctx.push_raw_str("<div class=\"gallery-box\">\n");
    match sources {
        Some(sources) => {
            for source in sources {
                render_item(ctx, size, file_source(&page, source), true);
            }
        }
        None => {
            for file in &ctx.handle().page_images {
                let source = FileSource::File2 {
                    page: Cow::Borrowed(&page),
                    file: Cow::Borrowed(file),
                };
                render_item(ctx, size, source, false);
            }
        }
    }
    ctx.push_raw_str("</div>");
}

/// A list line names a file of this page, `page/file`, or a URL.
fn file_source<'a>(page: &'a str, source: &'a str) -> FileSource<'a> {
    if source.contains("://") {
        return FileSource::Url(Cow::Borrowed(source));
    }
    match source.trim_start_matches('/').split_once('/') {
        Some((page, file)) => FileSource::File2 {
            page: Cow::Borrowed(page),
            file: Cow::Borrowed(file),
        },
        None => FileSource::File2 {
            page: Cow::Borrowed(page),
            file: Cow::Borrowed(source),
        },
    }
}

fn render_item(ctx: &mut HtmlContext, size: &str, source: FileSource, listed: bool) {
    let Some(url) = ctx
        .handle()
        .get_file_link(&source, ctx.info(), ctx.settings())
    else {
        return;
    };

    ctx.push_raw_str("<div class=\"gallery-item ");
    ctx.push_escaped(size);
    ctx.push_raw_str("\">\n<table>\n<tr>\n<td><a href=\"");
    ctx.push_escaped(&url);
    if listed {
        ctx.push_raw_str("\" class=\"with-lb");
    }
    ctx.push_raw_str("\"><img src=\"");
    ctx.push_escaped(&url);
    ctx.push_raw_str("\" alt=\"\"");
    if listed {
        ctx.push_raw_str(" class=\"gallery-image-size-");
        ctx.push_escaped(size);
        ctx.push_raw_str("\"");
    }
    if let Some(style) = thumbnail_style(size) {
        ctx.push_raw_str(" style=\"");
        ctx.push_raw_str(style);
        ctx.push_raw_str("\"");
    }
    ctx.push_raw_str(" /></a></td>\n</tr>\n</table>\n</div>\n");
}

/// Wikidot links resized copies (`local--resized-images/.../<size>.jpg`),
/// scaled to fit a box or, for "square", cropped to it. There is no resizer
/// here, so the original file is shown at that size instead. Retire this when
/// resized images are served.
fn thumbnail_style(size: &str) -> Option<&'static str> {
    match size {
        "square" => Some("width: 75px; height: 75px; object-fit: cover;"),
        "thumbnail" => Some("width: 100px; height: 100px; object-fit: contain;"),
        "small" => Some("width: 240px; height: 240px; object-fit: contain;"),
        "medium" => Some("width: 500px; height: 500px; object-fit: contain;"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::data::PageInfo;
    use crate::layout::Layout;
    use crate::render::Handle;
    use crate::render::html::HtmlRender;
    use crate::settings::{WikitextMode, WikitextSettings};
    use std::borrow::Cow;

    fn render(source: &str, page_images: &[&str]) -> String {
        let page_info = PageInfo {
            page: Cow::Borrowed("icons"),
            ..PageInfo::dummy()
        };
        let settings = WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot);
        let mut text = source.to_owned();
        crate::preprocess(&mut text);
        let tokens = crate::tokenize(&text);
        // Errors are not checked: a stray [[/gallery]] is one, as on Wikidot.
        let (tree, _errors) = crate::parse(&tokens, &page_info, &settings).into();
        let handle = Handle {
            page_images: page_images.iter().map(|name| name.to_string()).collect(),
            ..Handle::default()
        };
        HtmlRender
            .render_with_handle(&tree, &page_info, &settings, handle)
            .body
    }

    #[test]
    fn bare_gallery_shows_page_images_like_wikidot() {
        // icons: `[[gallery]]` over the page's attachments, in Wikidot's order.
        assert_eq!(
            render("[[gallery]]", &["icon_abigael.jpg", "icon_adal.jpg"]),
            "<div class=\"gallery-box\">\n\
             <div class=\"gallery-item thumbnail\">\n<table>\n<tr>\n\
             <td><a href=\"/-/file/icons/icon_abigael.jpg\"><img src=\"/-/file/icons/icon_abigael.jpg\" alt=\"\" style=\"width: 100px; height: 100px; object-fit: contain;\" /></a></td>\n\
             </tr>\n</table>\n</div>\n\
             <div class=\"gallery-item thumbnail\">\n<table>\n<tr>\n\
             <td><a href=\"/-/file/icons/icon_adal.jpg\"><img src=\"/-/file/icons/icon_adal.jpg\" alt=\"\" style=\"width: 100px; height: 100px; object-fit: contain;\" /></a></td>\n\
             </tr>\n</table>\n</div>\n\
             </div>",
        );
    }

    #[test]
    fn gallery_size_selects_the_thumbnail_box() {
        // badges: `[[gallery size="square"]]`; unknown sizes are "thumbnail".
        let square = render("[[gallery size=\"square\"]]", &["badge_class_druid.png"]);
        assert!(
            square.contains(
                "<div class=\"gallery-item square\">\n<table>\n<tr>\n\
                 <td><a href=\"/-/file/icons/badge_class_druid.png\"><img src=\"/-/file/icons/badge_class_druid.png\" alt=\"\" style=\"width: 75px; height: 75px; object-fit: cover;\" />"
            ),
            "{square}"
        );
        let unknown = render("[[gallery size=\"huge\"]]", &["a.png"]);
        assert!(unknown.contains("gallery-item thumbnail"), "{unknown}");
    }

    #[test]
    fn listed_gallery_shows_its_files_in_order() {
        // pet-icons: a `: file` list, rendered with Wikidot's lightbox classes.
        let html = render(
            "[[gallery]]\n: pet-icon_elekk-gray.jpg\n: pet-icon_elekk-brown.jpg\n[[/gallery]]\n\nAfter",
            &["ignored.jpg"],
        );
        assert_eq!(
            html,
            "<div class=\"gallery-box\">\n\
             <div class=\"gallery-item thumbnail\">\n<table>\n<tr>\n\
             <td><a href=\"/-/file/icons/pet-icon_elekk-gray.jpg\" class=\"with-lb\"><img src=\"/-/file/icons/pet-icon_elekk-gray.jpg\" alt=\"\" class=\"gallery-image-size-thumbnail\" style=\"width: 100px; height: 100px; object-fit: contain;\" /></a></td>\n\
             </tr>\n</table>\n</div>\n\
             <div class=\"gallery-item thumbnail\">\n<table>\n<tr>\n\
             <td><a href=\"/-/file/icons/pet-icon_elekk-brown.jpg\" class=\"with-lb\"><img src=\"/-/file/icons/pet-icon_elekk-brown.jpg\" alt=\"\" class=\"gallery-image-size-thumbnail\" style=\"width: 100px; height: 100px; object-fit: contain;\" /></a></td>\n\
             </tr>\n</table>\n</div>\n\
             </div><p>After</p>",
        );
    }

    #[test]
    fn gallery_without_list_lines_stays_bare() {
        // Wikidot's list form needs `: file` lines; the rest is page text.
        let html = render("[[gallery]]\nCaption\n[[/gallery]]", &["a.png"]);
        assert!(html.contains("/-/file/icons/a.png"), "{html}");
        assert!(html.contains("Caption"), "{html}");
    }
}
