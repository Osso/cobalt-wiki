/*
 * services/outdate.rs
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

use super::prelude::*;
use crate::futures::StreamExt;
use crate::models::page::{self, Entity as Page, Model as PageModel};
use crate::models::page_category::{self, Entity as PageCategory};
use crate::services::render::{ListingSubject, listing_pages_affected_by};
use crate::services::{JobService, LinkService, PageService, SiteService};
use crate::types::{ConnectionType, PageId, PageOrder};
use crate::utils::split_category_name;
use ref_map::*;
use sea_orm::FromQueryResult;
use std::collections::{BTreeSet, HashSet};

#[derive(Debug)]
pub struct OutdateService;

impl OutdateService {
    pub async fn process_page_edit(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        page_id: i64,
        slug: &str,
    ) -> Result<()> {
        let make_error = || {
            Error::new(
                format!(
                    "failed to run outdater for edit of page '{}' (ID {}) on site ID {}",
                    slug, page_id, site_id,
                ),
                ErrorType::PageOutdater,
            )
        };

        let (category_slug, page_slug) = split_category_name(slug);
        let (result1, result2, result3) = join!(
            Self::outdate_outgoing_includes(ctx, site_id, page_id),
            Self::outdate_templates(ctx, site_id, category_slug, page_slug),
            Self::outdate_nav_pages(ctx, site_id, slug),
        );
        raise_multiple!(result1, result2, result3; make_error);

        Ok(())
    }

    /// Performs outdating tasks for a page being created or deleted here.
    pub async fn process_page_displace(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        page_id: i64,
        slug: &str,
    ) -> Result<()> {
        let make_error = || {
            Error::new(
                format!(
                    "failed to run outdater for displacement of page '{}' (ID {}) on site ID {}",
                    slug, page_id, site_id,
                ),
                ErrorType::PageOutdater,
            )
        };

        let (result1, result2) = join!(
            Self::process_page_edit(ctx, site_id, page_id, slug),
            Self::outdate_incoming_links(ctx, site_id, page_id),
        );
        raise_multiple!(result1, result2; make_error);

        Ok(())
    }

    pub async fn process_page_move(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        page_id: i64,
        old_slug: &str,
        new_slug: &str,
    ) -> Result<()> {
        let make_error = || {
            Error::new(
                format!(
                    "failed to run outdater for move of page ID {} from '{}' to '{}' on site ID {}",
                    page_id, old_slug, new_slug, site_id,
                ),
                ErrorType::PageOutdater,
            )
        };

        // In terms of outdating, a move is equivalent to
        // deleting at the old page location and
        // creating at the new page location.
        let (result1, result2) = join!(
            Self::process_page_displace(ctx, site_id, page_id, new_slug),
            Self::process_page_displace(ctx, site_id, page_id, old_slug),
        );
        raise_multiple!(result1, result2; make_error);

        Ok(())
    }

    /// Queues the given dependent pages for re-rendering.
    ///
    /// Every page whose output depends on a change is found here, when the
    /// change happens: includes are recorded transitively, and templates and
    /// listings are recorded on each page using them. So dependent jobs queue
    /// nothing further. The exception is a dependent that is a navigation page:
    /// every page using it stores its own compiled copy of the bar, and those
    /// are queued here too.
    async fn outdate_pages(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        page_ids: BTreeSet<i64>,
    ) -> Result<()> {
        if page_ids.is_empty() {
            return Ok(());
        }

        let make_error = || {
            Error::new(
                format!(
                    "failed to run outdater on {} pages on site ID {site_id}",
                    page_ids.len(),
                ),
                ErrorType::PageOutdater,
            )
        };

        let nav_slugs = Self::nav_page_slugs(ctx, site_id)
            .await
            .or_raise(make_error)?;

        for &page_id in &page_ids {
            let page = PageService::get_direct(ctx, page_id, false)
                .await
                .or_raise(make_error)?;

            let id = PageId::from_page_model(&page);
            JobService::queue_rerender_page(ctx, id)
                .await
                .or_raise(make_error)?;

            if nav_slugs.contains(&page.slug) {
                Self::outdate_nav_pages(ctx, site_id, &page.slug)
                    .await
                    .or_raise(make_error)?;
            }
        }

        Ok(())
    }

    /// Slugs of the site's and its categories' navigation pages.
    async fn nav_page_slugs(
        ctx: &ServiceContext<'_>,
        site_id: i64,
    ) -> Result<HashSet<String>> {
        let make_error = || {
            Error::new(
                format!("failed to get navigation pages of site ID {site_id}"),
                ErrorType::PageOutdater,
            )
        };
        let site = SiteService::get(ctx, Reference::Id(site_id))
            .await
            .or_raise(make_error)?;
        let categories: Vec<(Option<String>, Option<String>)> = PageCategory::find()
            .select_only()
            .column(page_category::Column::TopBarPage)
            .column(page_category::Column::SideBarPage)
            .filter(page_category::Column::SiteId.eq(site_id))
            .into_tuple()
            .all(ctx.transaction())
            .await
            .or_raise(make_error)?;

        Ok([site.top_bar_page, site.side_bar_page]
            .into_iter()
            .chain(
                categories
                    .into_iter()
                    .flat_map(|(top, side)| [top, side])
                    .flatten(),
            )
            .filter(|slug| !slug.is_empty())
            .collect())
    }

    /// Rerender the listing pages that could show `page_id` in any of the given
    /// states (`(slug, tags)` before and after a change).
    pub async fn outdate_listings(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        page_id: i64,
        states: &[(&str, &[String])],
    ) -> Result<()> {
        let make_error = || {
            Error::new(
                format!(
                    "failed to run outdater for listings of page ID {page_id} on site ID {site_id}"
                ),
                ErrorType::PageOutdater,
            )
        };
        let subjects: Vec<_> = states
            .iter()
            .map(|&(slug, tags)| {
                let (category, name) = split_category_name(slug);
                ListingSubject {
                    category,
                    name,
                    tags,
                }
            })
            .collect();
        let ids = listing_pages_affected_by(ctx, site_id, &subjects)
            .await
            .or_raise(make_error)?
            .into_iter()
            .filter(|&id| id != page_id)
            .collect();
        Self::outdate_pages(ctx, site_id, ids)
            .await
            .or_raise(make_error)
    }

    pub async fn outdate_incoming_links(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        page_id: i64,
    ) -> Result<()> {
        const CONNECTION_TYPES: &[ConnectionType] = &[ConnectionType::Link];

        let make_error = || {
            Error::new(
                format!(
                    "failed to run outdater for all pages that link to page ID {}",
                    page_id,
                ),
                ErrorType::PageOutdater,
            )
        };

        let ids = LinkService::get_to(ctx, page_id, Some(CONNECTION_TYPES))
            .await
            .or_raise(make_error)?
            .connections
            .iter()
            .map(|connection| connection.from_page_id)
            .filter(|id| *id != page_id)
            .collect();
        Self::outdate_pages(ctx, site_id, ids)
            .await
            .or_raise(make_error)?;

        Ok(())
    }

    pub async fn outdate_outgoing_includes(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        page_id: i64,
    ) -> Result<()> {
        const CONNECTION_TYPES: &[ConnectionType] = &[
            ConnectionType::IncludeMessy,
            ConnectionType::IncludeElements,
            ConnectionType::Component,
        ];

        let make_error = || {
            Error::new(
                format!(
                    "failed to run outdater for all pages which include page ID {}",
                    page_id,
                ),
                ErrorType::PageOutdater,
            )
        };

        let ids = LinkService::get_to(ctx, page_id, Some(CONNECTION_TYPES))
            .await
            .or_raise(make_error)?
            .connections
            .iter()
            .map(|connection| connection.from_page_id)
            .filter(|id| *id != page_id)
            .collect();
        Self::outdate_pages(ctx, site_id, ids)
            .await
            .or_raise(make_error)?;
        Ok(())
    }

    pub async fn outdate_templates(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        category_slug: &str,
        page_slug: &str,
    ) -> Result<()> {
        let config = ctx.config();

        let make_error = || {
            Error::new(
                format!(
                    "failed to run outdater for all pages in category '{}' on site ID {} using page '{}' as a template",
                    category_slug, site_id, page_slug,
                ),
                ErrorType::PageOutdater,
            )
        };

        // If a template page has been updated,
        // we need to recompile everything in that category.
        if page_slug == config.blueprint_page_template {
            let category_select = if category_slug == "_default" {
                // If the category is _default, we need to recompile everything.
                // All other categories may inherit from _default.
                //
                // Specifying "None" here means that we aren't filtering by category.
                None
            } else {
                // Otherwise, filter by whatever category slug we have here.
                Some(category_slug.into())
            };

            let pages = PageService::get_all(
                ctx,
                site_id,
                category_select,
                Some(false),
                PageOrder::default(),
            )
            .await
            .or_raise(make_error)?;

            let ids = pages.into_iter().map(|page| page.page_id).collect();
            Self::outdate_pages(ctx, site_id, ids)
                .await
                .or_raise(make_error)?;
        }

        Ok(())
    }

    /// Determines if the page being updated is used as an nav page anywhere.
    /// If so, all pages using this as a nav page should have their nav pages rebuilt.
    pub async fn outdate_nav_pages(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        slug: &str,
    ) -> Result<()> {
        let make_error = || {
            Error::new(
                format!(
                    "failed to run nav-only outdater for all pages using page '{}' on site ID {} as a nav page",
                    slug, site_id,
                ),
                ErrorType::PageOutdater,
            )
        };

        // If this is the nav page for the site, then outdate everything
        // Nothing else needs to be done.
        let site = SiteService::get(ctx, Reference::Id(site_id))
            .await
            .or_raise(make_error)?;

        if site.top_bar_page == slug || site.side_bar_page == slug {
            Self::outdate_nav_site(ctx, site_id)
                .await
                .or_raise(make_error)?;
            return Ok(());
        }

        // If this is the nav page for a category, then outdate all
        // the pages in that category. Note that multiple categories
        // can use the same nav pages.
        let txn = ctx.transaction();
        let category_ids = PageCategory::find()
            .select_only()
            .column(page_category::Column::CategoryId)
            .filter(
                Condition::any()
                    .add(page_category::Column::TopBarPage.eq(slug))
                    .add(page_category::Column::SideBarPage.eq(slug)),
            )
            .into_tuple()
            .all(txn)
            .await
            .or_raise(make_error)?;

        for category_id in category_ids {
            Self::outdate_nav_category(ctx, site_id, category_id)
                .await
                .or_raise(make_error)?;
        }

        Ok(())
    }

    /// Outdates the nav pages of every page on the site.
    pub async fn outdate_nav_site(ctx: &ServiceContext<'_>, site_id: i64) -> Result<()> {
        info!("Outdating all pages on site ID {site_id}");

        let make_error = || {
            Error::new(
                format!(
                    "failed to run nav-only outdater for all pages on site ID {}",
                    site_id,
                ),
                ErrorType::PageOutdater,
            )
        };

        #[derive(FromQueryResult)]
        struct Row {
            site_id: i64,
            page_category_id: i64,
            page_id: i64,
        }

        let txn = ctx.transaction();
        let mut rows = Page::find()
            .select_only()
            .column(page::Column::SiteId)
            .column(page::Column::PageCategoryId)
            .column(page::Column::PageId)
            .filter(
                Condition::all()
                    .add(page::Column::SiteId.eq(site_id))
                    .add(page::Column::DeletedAt.is_null()),
            )
            .into_model::<Row>()
            .stream(txn)
            .await
            .or_raise(make_error)?;

        while let Some(row) = rows.next().await {
            let Row {
                site_id,
                page_category_id: category_id,
                page_id,
            } = row.or_raise(make_error)?;

            JobService::queue_rerender_nav_page(
                ctx,
                PageId {
                    site_id,
                    category_id,
                    page_id,
                },
            )
            .await
            .or_raise(make_error)?;
        }

        Ok(())
    }

    /// Outdates the nav pages of all pages in the given category.
    pub async fn outdate_nav_category(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        category_id: i64,
    ) -> Result<()> {
        let make_error = || {
            Error::new(
                format!(
                    "failed to run nav-only outdater for all pages in category ID {} on site ID {}",
                    category_id, site_id,
                ),
                ErrorType::PageOutdater,
            )
        };

        let txn = ctx.transaction();
        let mut rows = Page::find()
            .select_only()
            .column(page::Column::PageId)
            .filter(
                Condition::all()
                    .add(page::Column::SiteId.eq(site_id))
                    .add(page::Column::PageCategoryId.eq(category_id))
                    .add(page::Column::DeletedAt.is_null()),
            )
            .into_tuple()
            .stream(txn)
            .await
            .or_raise(make_error)?;

        while let Some(row) = rows.next().await {
            let page_id = row.or_raise(make_error)?;

            JobService::queue_rerender_nav_page(
                ctx,
                PageId {
                    site_id,
                    category_id,
                    page_id,
                },
            )
            .await
            .or_raise(make_error)?;
        }

        Ok(())
    }
}
