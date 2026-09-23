/*
 * services/render/service.rs
 *
 * DEEPWELL - Wikijump API provider and database manager
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

use super::list_pages::ListingFilter;
use super::prelude::*;
use crate::hash::TextHash;
use crate::services::TextService;
use crate::services::settings::{NavigationPageWikitext, SettingsService};
use crate::services::text_block::{
    MIME_HTML, TextBlock, TextBlockService, mime_for_language,
};
use crate::types::{PageId, TextBlockType};
use ftml::prelude::*;
use ftml::tree::CodeBlock;
use std::sync::LazyLock;
use tokio::time::timeout;

/// Identifies what compiled stored HTML: FTML's version plus this Deepwell build
/// (`DEEPWELL_BUILD_ID`, set by the Nix package from the source hash). A page
/// compiled by another build is rerendered when it is next viewed.
pub static COMPILED_GENERATOR: LazyLock<String> =
    LazyLock::new(|| match option_env!("DEEPWELL_BUILD_ID") {
        Some(build) => format!("{} {build}", *FTML_VERSION),
        None => FTML_VERSION.to_string(),
    });

/// URL arguments that change a page body: the ListPages page (`/p/N`) and
/// the PagesByTag tag (`/tag/NAME`). The stored render uses the defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyArguments {
    pub list_page: usize,
    pub tag: Option<String>,
}

impl Default for BodyArguments {
    fn default() -> Self {
        BodyArguments {
            list_page: 1,
            tag: None,
        }
    }
}

#[derive(Debug)]
pub struct RenderService;

impl RenderService {
    pub async fn render(
        ctx: &ServiceContext<'_>,
        wikitext: String,
        page_info: &PageInfo<'_>,
        settings: &WikitextSettings,
    ) -> Result<RenderOutput> {
        let wikitext_len = wikitext.len();
        let make_error = || {
            Error::new(
                format!(
                    "failed to run parse and render (wikitext {} bytes, info {:?}, settings {:?})",
                    wikitext_len, page_info, settings,
                ),
                ErrorType::Render,
            )
        };

        let RenderInnerOutput {
            html_output,
            errors,
            compiled_hash,
        } = Self::render_inner(ctx, wikitext, page_info, settings, None)
            .await
            .or_raise(make_error)?;

        Ok(RenderOutput {
            html_output,
            errors,
            compiled_hash,
            compiled_at: now(),
            compiled_generator: COMPILED_GENERATOR.clone(),
        })
    }

    pub async fn render_page(
        ctx: &ServiceContext<'_>,
        wikitext: String,
        page_info: &PageInfo<'_>,
        layout: Layout,
        PageId {
            site_id,
            category_id,
            page_id,
        }: PageId,
    ) -> Result<RenderPageOutput> {
        let page_settings = WikitextSettings::from_mode(WikitextMode::Page, layout);
        let nav_settings = WikitextSettings::from_mode(WikitextMode::PageNav, layout);

        let wikitext_len = wikitext.len();
        let make_error = || {
            Error::new(
                format!(
                    "failed to run parse and render for page ID {} in site ID {} (wikitext {} bytes, info {:?}, layout {})",
                    page_id,
                    site_id,
                    wikitext_len,
                    page_info,
                    layout.description(),
                ),
                ErrorType::Render,
            )
        };

        let RenderInnerOutput {
            html_output,
            errors,
            compiled_hash: compiled_body_html_hash,
        } = Self::render_inner(ctx, wikitext, page_info, &page_settings, Some(page_id))
            .await
            .or_raise(make_error)?;

        let NavigationPageWikitext {
            top_bar_page_wikitext,
            side_bar_page_wikitext,
        } = SettingsService::get_nav_page_wikitext(ctx, site_id, Some(category_id))
            .await
            .or_raise(make_error)?;

        let render_nav_page = |wikitext| async {
            match wikitext {
                Some(wikitext) => {
                    // We are providing page_id = None because that will trigger the steps
                    // to update text blocks, which is incorrect for navigation pages.
                    //
                    // Also note that the page_info for nav pages is the page being displayed,
                    // not the nav pages themselves. This means that any variables or blocks
                    // which depend on the current page (e.g. page slug, tags), which reflect
                    // the page being viewed.
                    let result =
                        Self::render_inner(ctx, wikitext, page_info, &nav_settings, None)
                            .await;

                    match result {
                        Ok(RenderInnerOutput { compiled_hash, .. }) => {
                            Ok(Some(compiled_hash))
                        }
                        Err(error) => Err(error),
                    }
                }

                // No nav page
                None => Ok(None),
            }
        };

        let (top_bar_render_result, side_bar_render_result) = join!(
            render_nav_page(top_bar_page_wikitext),
            render_nav_page(side_bar_page_wikitext),
        );
        let (compiled_top_bar_html_hash, compiled_side_bar_html_hash) =
            raise_multiple!(top_bar_render_result, side_bar_render_result; make_error);

        Ok(RenderPageOutput {
            html_output,
            errors,
            compiled_body_html_hash,
            compiled_top_bar_html_hash,
            compiled_side_bar_html_hash,
            compiled_at: now(),
            compiled_generator: COMPILED_GENERATOR.clone(),
        })
    }

    /// Render a page's body for URL arguments other than the stored defaults
    /// without storing anything.
    pub async fn render_page_view(
        ctx: &ServiceContext<'_>,
        wikitext: String,
        page_info: &PageInfo<'_>,
        layout: Layout,
        body: &BodyArguments,
    ) -> Result<String> {
        let settings = WikitextSettings::from_mode(WikitextMode::Page, layout);
        let rendered =
            Self::render_html(ctx, wikitext, page_info, &settings, Some(body)).await?;
        Ok(rendered.html_output.body)
    }

    /// Render and store the output: compiled HTML always, text blocks for pages.
    async fn render_inner(
        ctx: &ServiceContext<'_>,
        wikitext: String,
        page_info: &PageInfo<'_>,
        settings: &WikitextSettings,
        page_id: Option<i64>,
    ) -> Result<RenderInnerOutput> {
        let make_error =
            || Error::new("failed to perform render operation", ErrorType::Render);
        let stored_body = BodyArguments::default();
        let RenderedHtml {
            html_output,
            errors,
            listings,
            html_blocks,
            code_blocks,
        } = Self::render_html(
            ctx,
            wikitext,
            page_info,
            settings,
            page_id.map(|_| &stored_body),
        )
        .await?;
        // Insert compiled HTML into text table
        let compiled_hash = TextService::create(ctx, html_output.body.clone())
            .await
            .or_raise(make_error)?;

        // Set up the hosted text blocks
        //
        // This only applies for published pages, in any other
        // rendering context and we should skip this step.

        if let Some(page_id) = page_id {
            // It's possible to render a page without doing text blocks
            // (e.g. blueprint pages), but all cases where text blocks
            // are done are pages.
            debug_assert_eq!(settings.mode, WikitextMode::Page);

            super::listing_deps::replace(ctx, page_id, &listings)
                .await
                .or_raise(make_error)?;

            // [[html]]
            let html_blocks: Vec<TextBlock> = html_blocks
                .iter()
                .map(|html| TextBlock {
                    text: html,
                    text_type: None,
                    mime: MIME_HTML,
                    name: None,
                })
                .collect();

            TextBlockService::add_blocks(ctx, page_id, TextBlockType::Html, &html_blocks)
                .await
                .or_raise(make_error)?;

            // [[code]]
            let code_blocks: Vec<TextBlock> = code_blocks
                .iter()
                .map(
                    |CodeBlock {
                         contents,
                         language,
                         name,
                     }| TextBlock {
                        text: contents,
                        text_type: language.as_deref(),
                        mime: mime_for_language(language),
                        name: name.as_deref(),
                    },
                )
                .collect();

            TextBlockService::add_blocks(ctx, page_id, TextBlockType::Code, &code_blocks)
                .await
                .or_raise(make_error)?;
        }

        // Build and return
        Ok(RenderInnerOutput {
            html_output,
            errors,
            compiled_hash,
        })
    }

    /// Preprocess, parse and render without storing anything. `body` is
    /// `Some` for a published page body (live template, URL arguments) and
    /// `None` for fragments such as navigation pages and previews.
    async fn render_html(
        ctx: &ServiceContext<'_>,
        mut wikitext: String,
        page_info: &PageInfo<'_>,
        settings: &WikitextSettings,
        body: Option<&BodyArguments>,
    ) -> Result<RenderedHtml> {
        let config = ctx.config();

        // We isolate the actual tasks for rendering,
        // allowing us to time it out if it takes too long.
        //
        // The preprocess step has to be distinct for borrowing reasons,
        // since we want to do the processing for non-ftml work
        // outside the timeout guards.

        let (tokens, included_pages, listings) =
            timeout(config.preprocess_timeout, async {
                let mut source = std::mem::take(&mut wikitext);
                if body.is_some() {
                    source =
                        super::live_template::apply_live_template(ctx, source, page_info)
                            .await?;
                }
                let source = super::show_to::strip_show_to_regions(source);
                let (expanded, included_pages) = super::includes::expand_includes(
                    ctx,
                    source,
                    &page_info.site,
                    settings,
                )
                .await?;
                let (listed, listings) = super::list_pages::expand_list_pages(
                    ctx,
                    expanded,
                    page_info,
                    body.map_or(1, |body| body.list_page),
                )
                .await?;
                wikitext = listed;
                wikitext = super::wikidot_comments::strip_comments(std::mem::take(
                    &mut wikitext,
                ));
                ftml::preprocess(&mut wikitext);
                Ok::<_, ExnError>((ftml::tokenize(&wikitext), included_pages, listings))
            })
            .await
            .or_raise(|| {
                Error::new(
                    "failed to preprocess and tokenize due to timeout",
                    ErrorType::RenderTimeout,
                )
            })??;

        let (tree, mut html_output, errors, lists_every_page) =
            timeout(config.render_timeout, async {
                let result = ftml::parse(&tokens, page_info, settings);
                let (tree, errors) = result.into();
                super::render_data::fetch_render_data(ctx, &tree, page_info, body)
                    .await
                    .map(|(handle, lists_every_page)| {
                        let html_output = HtmlRender
                            .render_with_handle(&tree, page_info, settings, handle);
                        (tree, html_output, errors, lists_every_page)
                    })
            })
            .await
            .or_raise(|| {
                Error::new(
                    "failed to parse and render due to timeout",
                    ErrorType::RenderTimeout,
                )
            })??;

        html_output.backlinks.included_pages.extend(included_pages);
        let mut listings = listings;
        if lists_every_page {
            listings.push(ListingFilter::every_page());
        }
        Ok(RenderedHtml {
            html_output,
            errors,
            listings,
            html_blocks: tree
                .html_blocks
                .iter()
                .map(|html| html.to_string())
                .collect(),
            code_blocks: tree.code_blocks.iter().map(CodeBlock::to_owned).collect(),
        })
    }
}

#[derive(Debug)]
struct RenderedHtml {
    html_output: HtmlOutput,
    errors: Vec<ParseError>,
    listings: Vec<ListingFilter>,
    html_blocks: Vec<String>,
    code_blocks: Vec<CodeBlock<'static>>,
}

#[derive(Debug)]
struct RenderInnerOutput {
    html_output: HtmlOutput,
    errors: Vec<ParseError>,
    compiled_hash: TextHash,
}
