/*
 * render/handle.rs
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

use crate::data::{KarmaLevel, PageInfo, UserInfo};
use crate::render::html::escape::escape;
use crate::settings::WikitextSettings;
use crate::tree::{FileSource, LinkLabel, LinkLocation, Module};
use crate::url::BuildSiteUrl;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use wikidot_normalize::normalize;

/// Site data the renderer needs, supplied by the embedding application.
#[derive(Debug, Default)]
pub struct Handle {
    pub page_titles: BTreeMap<(String, String), String>,

    /// Visible tags with their page counts, in the site's tag order (TagCloud).
    pub tag_weights: Vec<(String, u64)>,

    /// The tag named in the URL and its pages as `(slug, title)` (PagesByTag).
    pub tagged_pages: Option<(String, Vec<(String, String)>)>,
}

impl Handle {
    pub fn render_module(&self, buffer: &mut String, module: &Module) {
        // Modules only render to HTML
        debug!("Rendering module '{}'", module.name());
        match module {
            // Wikidot's markup without its join dialog script, which this site lacks.
            Module::Join { button_text, .. } => {
                buffer.push_str("<div class=\"join-box\"><a href=\"javascript:;\">");
                escape(buffer, button_text.as_deref().unwrap_or("Join"));
                buffer.push_str("</a></div>");
            }
            // Wikidot's markup; the site's own script handles the submit.
            Module::NewPage {
                category,
                button_text,
                size,
                format,
            } => {
                buffer.push_str(
                    "<div class=\"new-page-box\" style=\"text-align: center; margin: 1em 0;\">\
                     <form action=\"dummy.html\" method=\"get\">\
                     <input class=\"text\" name=\"pageName\" type=\"text\" size=\"",
                );
                escape(buffer, size.as_deref().unwrap_or("30"));
                buffer.push_str(
                    "\" maxlength=\"128\" style=\"margin: 1px\"/>\n\
                     <input type=\"submit\" class=\"button\" value=\"",
                );
                escape(buffer, button_text.as_deref().unwrap_or("create page"));
                buffer.push_str("\" style=\"margin: 1px;\"/>");
                if let Some(category) = category {
                    buffer.push_str(
                        "<input type=\"hidden\" name=\"categoryName\" value=\"",
                    );
                    let mut category = category.to_string();
                    normalize(&mut category);
                    escape(buffer, &category);
                    buffer.push_str("\"/>");
                }
                if let Some(format) = format {
                    buffer.push_str("<input type=\"hidden\" name=\"format\" value=\"");
                    escape(buffer, format);
                    buffer.push_str("\"/>");
                }
                buffer.push_str("</form></div>");
            }
            Module::TagCloud {
                limit,
                target,
                min_font_size,
                max_font_size,
                min_color,
                max_color,
            } => render_tag_cloud(
                buffer,
                &self.tag_weights,
                TagCloudOptions::parse(
                    limit.as_deref(),
                    target.as_deref(),
                    (min_font_size.as_deref(), max_font_size.as_deref()),
                    (min_color.as_deref(), max_color.as_deref()),
                ),
            ),
            Module::PagesByTag => {
                if let Some((tag, pages)) = &self.tagged_pages {
                    render_pages_by_tag(buffer, tag, pages);
                }
            }
            _ => str_write!(buffer, "<p>TODO: module {}</p>", module.name()),
        }
    }

    pub fn get_page_title(&self, site: &str, page: &str) -> Option<String> {
        self.page_titles
            .get(&(site.to_owned(), page.to_owned()))
            .cloned()
    }

    pub fn get_page_exists(&self, _site: &str, _page: &str) -> bool {
        debug!("Checking page existence");

        // For testing
        #[cfg(test)]
        if _page == "missing" {
            return false;
        }

        // TODO
        true
    }

    pub fn get_user_info<'a>(&self, name: &'a str) -> Option<UserInfo<'a>> {
        debug!("Fetching user info (name '{name}')");
        let mut info = UserInfo::dummy();
        info.user_name = cow!(name);
        info.user_profile_url = Cow::Owned(format!("/user:info/{name}"));
        Some(info)
    }

    pub fn get_file_link<'a>(
        &self,
        source: &FileSource<'a>,
        info: &PageInfo,
        settings: &WikitextSettings,
    ) -> Option<Cow<'a, str>> {
        let (site, page, file): (&str, &str, &str) = match source {
            FileSource::Url(url) => return Some(Cow::clone(url)),
            FileSource::File1 { .. }
            | FileSource::File2 { .. }
            | FileSource::File3 { .. }
                if !settings.allow_local_paths =>
            {
                warn!("Specified path file source when local paths are disabled");
                return None;
            }
            FileSource::File1 { file } => (&info.site, &info.page, file),
            FileSource::File2 { page, file } => (&info.site, page, file),
            FileSource::File3 { site, page, file } => (site, page, file),
        };

        if site == info.site.as_ref() {
            return Some(Cow::Owned(format!("/-/file/{page}/{file}")));
        }

        Some(Cow::Owned(format!(
            "https://{site}.wjfiles.com/local--files/{page}/{file}",
        )))
    }

    pub fn get_link_label<F>(
        &self,
        site: &str,
        link: &LinkLocation,
        label: &LinkLabel,
        f: F,
    ) where
        F: FnOnce(&str),
    {
        let page_title;
        let label_text = match label {
            LinkLabel::Text(text) | LinkLabel::Slug(text) => text,
            LinkLabel::Url => match link {
                LinkLocation::Url(url) => url.as_ref(),
                LinkLocation::Page(_) => {
                    panic!("Requested a URL link label for a page");
                }
            },
            LinkLabel::Page => match link {
                LinkLocation::Page(page_ref) => {
                    let (site, page, _) = page_ref.fields_or(site);
                    page_title = match self.get_page_title(site, page) {
                        Some(title) => title,
                        None => page_ref.to_string(),
                    };

                    &page_title
                }
                LinkLocation::Url(_) => {
                    panic!("Requested a page title link label for a URL");
                }
            },
        };

        f(label_text);
    }

    pub fn get_karma_style(&self, karma: KarmaLevel) -> &'static str {
        // TODO replace these with inline data image URIs
        match karma {
            KarmaLevel::Zero => {
                "background-image: url(https://www.wikidot.com/userkarma.php?u=8976177)"
            }
            KarmaLevel::One => {
                "background-image: url(https://www.wikidot.com/userkarma.php?u=172570)"
            }
            KarmaLevel::Two => {
                "background-image: url(https://www.wikidot.com/userkarma.php?u=172952)"
            }
            KarmaLevel::Three => {
                "background-image: url(https://www.wikidot.com/userkarma.php?u=172904)"
            }
            KarmaLevel::Four => {
                "background-image: url(https://www.wikidot.com/userkarma.php?u=6040770)"
            }
            KarmaLevel::Five => {
                "background-image: url(https://www.wikidot.com/userkarma.php?u=4598089)"
            }
        }
    }

    pub fn get_message(&self, language: &str, message: &str) -> &'static str {
        debug!("Fetching message (language {language}, key {message})");

        let _ = language;

        // TODO
        match message {
            "button-copy-clipboard" => "Copy to Clipboard",
            "collapsible-open" => "+ open block",
            "collapsible-hide" => "- hide block",
            "table-of-contents" => "Table of Contents",
            "footnote" => "Footnote",
            "footnote-block-title" => "Footnotes",
            "bibliography-reference" => "Reference",
            "bibliography-block-title" => "Bibliography",
            "bibliography-cite-not-found" => "Bibliography item not found",
            "image-context-bad" => "No images in this context",
            "user-missing-pre" => "",
            "user-missing-post" => " does not match any existing user name",
            _ => {
                error!("Unknown message requested (key {message})");
                "?"
            }
        }
    }

    pub fn post_html(&self, info: &PageInfo, html: &str) -> String {
        debug!("Submitting HTML to create iframe-able snippet");

        let _ = info;
        let _ = html;

        // TODO
        str!("https://example.com/")
    }

    pub fn post_code(&self, index: NonZeroUsize, code: &str) {
        debug!("Submitting code snippet (index {})", index.get());

        let _ = index;
        let _ = code;

        // TODO
    }
}

impl BuildSiteUrl for Handle {
    fn build_url(&self, site: &str, path: &str, extra: Option<&str>) -> String {
        // TODO make this a parser setting
        // get url of wikijump instance here

        // TODO
        let extra = extra.unwrap_or("");
        format!("https://{site}.wikijump.com/{path}{extra}")
    }
}

/// Wikidot's TagCloud parameters, with its defaults and error messages.
struct TagCloudOptions {
    limit: usize,
    href: String,
    font_sizes: (u32, u32, &'static str),
    colors: ([u32; 3], [u32; 3]),
}

impl TagCloudOptions {
    fn parse(
        limit: Option<&str>,
        target: Option<&str>,
        font_sizes: (Option<&str>, Option<&str>),
        colors: (Option<&str>, Option<&str>),
    ) -> Result<Self, &'static str> {
        let href = match target {
            None | Some("") => "/system:page-tags/tag/".to_owned(),
            Some(target) => format!("/{}/tag/", target.trim_matches('/')),
        };
        let font_sizes = match font_sizes {
            (Some(min), Some(max)) => {
                let (small, unit) = parse_font_size(min)
                    .ok_or("Unsupported format for font size. Use px, em or %.")?;
                let (big, big_unit) = parse_font_size(max)
                    .ok_or("Unsupported format for font size. Use px, em or %.")?;
                if unit != big_unit {
                    return Err(
                        "Format for minFontSize and maxFontSize must be the same (px, em or %).",
                    );
                }
                (small, big, unit)
            }
            _ => (100, 300, "%"),
        };
        let colors = match colors {
            (Some(min), Some(max)) => match (parse_color(min), parse_color(max)) {
                (Some(small), Some(big)) => (small, big),
                _ => {
                    return Err(
                        "Unsupported color format. Use \"RRR,GGG,BBB\" for Red,Green,Blue each within 0-255 range.",
                    );
                }
            },
            _ => ([128, 128, 192], [64, 64, 128]),
        };
        let limit = limit
            .and_then(|limit| limit.trim().parse().ok())
            .filter(|&limit| limit > 0)
            .unwrap_or(50);
        Ok(TagCloudOptions {
            limit,
            href,
            font_sizes,
            colors,
        })
    }
}

fn parse_font_size(value: &str) -> Option<(u32, &'static str)> {
    let unit = ["%", "em", "px"]
        .into_iter()
        .find(|unit| value.ends_with(unit))?;
    let number = &value[..value.len() - unit.len()];
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((number.parse().ok()?, unit))
}

fn parse_color(value: &str) -> Option<[u32; 3]> {
    let parts: Vec<u32> = value
        .split(',')
        .map(|part| {
            (!part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| part.parse().ok())
                .flatten()
        })
        .collect::<Option<_>>()?;
    parts.try_into().ok()
}

/// PHP's `round()`: halves away from zero.
fn php_round(value: f64) -> u32 {
    value.round() as u32
}

/// PHP's `rawurlencode()`, as Smarty's `escape:'url'`.
fn raw_url_encode(buffer: &mut String, value: &str) {
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                buffer.push(char::from(byte))
            }
            _ => str_write!(buffer, "%{byte:02X}"),
        }
    }
}

fn render_tag_cloud(
    buffer: &mut String,
    tag_weights: &[(String, u64)],
    options: Result<TagCloudOptions, &'static str>,
) {
    let options = match options {
        Ok(options) => options,
        Err(message) => {
            buffer.push_str("<div class=\"error-block\">");
            escape(buffer, message);
            buffer.push_str("</div>");
            return;
        }
    };
    let tags = &tag_weights[..tag_weights.len().min(options.limit)];
    if tags.is_empty() {
        buffer.push_str(
            "<p>It seems you have no tags attached to pages. To attach a tag simply click on the <em>tags</em> button at the bottom of any page.</p>",
        );
        return;
    }
    let min = tags.iter().map(|(_, weight)| *weight).min().unwrap_or(0);
    let max = tags.iter().map(|(_, weight)| *weight).max().unwrap_or(0);
    let (small, big, unit) = options.font_sizes;
    let (color_small, color_big) = options.colors;
    let scale = |from: u32, to: u32, share: f64| {
        php_round(f64::from(from) + (f64::from(to) - f64::from(from)) * share)
    };
    buffer.push_str("<div class=\"pages-tag-cloud-box\">");
    for (tag, weight) in tags {
        let share = match max - min {
            0 => 0.0,
            range => (weight - min) as f64 / range as f64,
        };
        buffer.push_str("\n<a class=\"tag\" href=\"");
        escape(buffer, &options.href);
        raw_url_encode(buffer, tag);
        str_write!(
            buffer,
            "\" style=\"font-size: {}{unit}; color: rgb({}, {}, {});\">",
            scale(small, big, share),
            scale(color_small[0], color_big[0], share),
            scale(color_small[1], color_big[1], share),
            scale(color_small[2], color_big[2], share),
        );
        escape(buffer, tag);
        buffer.push_str("</a>");
    }
    buffer.push_str("\n</div>");
}

fn render_pages_by_tag(buffer: &mut String, tag: &str, pages: &[(String, String)]) {
    buffer.push_str("<a name=\"pages\"></a><h2>List of pages tagged with <em>");
    escape(buffer, tag);
    buffer.push_str("</em>:</h2>");
    if pages.is_empty() {
        buffer.push_str("<p>Somehow no pages have been found...</p>");
        return;
    }
    buffer.push_str("<div class=\"pages-list\" id=\"tagged-pages-list\">");
    for (slug, title) in pages {
        buffer
            .push_str("<div class=\"pages-list-item\"><div class=\"title\"><a href=\"/");
        escape(buffer, slug);
        buffer.push_str("\">");
        escape(buffer, if title.is_empty() { slug } else { title });
        buffer.push_str("</a></div></div>");
    }
    buffer.push_str("</div>");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag_cloud(handle: &Handle, module: Module) -> String {
        let mut buffer = String::new();
        handle.render_module(&mut buffer, &module);
        buffer
    }

    fn cobalt_tag_cloud() -> Module<'static> {
        // system:page-tags on cobalt-company.wikidot.com
        Module::TagCloud {
            limit: Some(cow!("200")),
            target: Some(cow!("system:page-tags")),
            min_font_size: Some(cow!("100%")),
            max_font_size: Some(cow!("225%")),
            min_color: None,
            max_color: None,
        }
    }

    #[test]
    fn tag_cloud_matches_wikidot_sizes_and_colors() {
        let handle = Handle {
            tag_weights: vec![
                ("@@".into(), 977),
                ("abigael".into(), 1),
                ("alli".into(), 1550),
            ],
            ..Handle::default()
        };
        assert_eq!(
            tag_cloud(&handle, cobalt_tag_cloud()),
            "<div class=\"pages-tag-cloud-box\">\n\
             <a class=\"tag\" href=\"/system:page-tags/tag/%40%40\" style=\"font-size: 179%; color: rgb(88, 88, 152);\">@@</a>\n\
             <a class=\"tag\" href=\"/system:page-tags/tag/abigael\" style=\"font-size: 100%; color: rgb(128, 128, 192);\">abigael</a>\n\
             <a class=\"tag\" href=\"/system:page-tags/tag/alli\" style=\"font-size: 225%; color: rgb(64, 64, 128);\">alli</a>\n\
             </div>",
        );
    }

    #[test]
    fn tag_cloud_scales_within_the_limited_tags() {
        let handle = Handle {
            tag_weights: vec![("a".into(), 2), ("b".into(), 4), ("c".into(), 100)],
            ..Handle::default()
        };
        let html = tag_cloud(
            &handle,
            Module::TagCloud {
                limit: Some(cow!("2")),
                target: None,
                min_font_size: None,
                max_font_size: None,
                min_color: None,
                max_color: None,
            },
        );
        assert!(
            html.contains("href=\"/system:page-tags/tag/b\" style=\"font-size: 300%")
        );
        assert!(!html.contains(">c</a>"), "{html}");
    }

    #[test]
    fn tag_cloud_reports_mixed_font_units() {
        let html = tag_cloud(
            &Handle::default(),
            Module::TagCloud {
                limit: None,
                target: None,
                min_font_size: Some(cow!("10px")),
                max_font_size: Some(cow!("2em")),
                min_color: None,
                max_color: None,
            },
        );
        assert_eq!(
            html,
            "<div class=\"error-block\">Format for minFontSize and maxFontSize must be the same (px, em or %).</div>",
        );
    }

    #[test]
    fn pages_by_tag_lists_tagged_pages_or_nothing_without_a_tag() {
        assert_eq!(tag_cloud(&Handle::default(), Module::PagesByTag), "");
        let handle = Handle {
            tagged_pages: Some((
                "abigael".into(),
                vec![
                    (
                        "writing:2026-04-03-a-letter-to-the-web".into(),
                        "(2026-04-03) A letter to the WEB".into(),
                    ),
                    ("character:abigael".into(), "Abigael Fenrhald".into()),
                ],
            )),
            ..Handle::default()
        };
        assert_eq!(
            tag_cloud(&handle, Module::PagesByTag),
            "<a name=\"pages\"></a><h2>List of pages tagged with <em>abigael</em>:</h2>\
             <div class=\"pages-list\" id=\"tagged-pages-list\">\
             <div class=\"pages-list-item\"><div class=\"title\"><a href=\"/writing:2026-04-03-a-letter-to-the-web\">(2026-04-03) A letter to the WEB</a></div></div>\
             <div class=\"pages-list-item\"><div class=\"title\"><a href=\"/character:abigael\">Abigael Fenrhald</a></div></div>\
             </div>",
        );
    }
}
