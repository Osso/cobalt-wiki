use super::prelude::*;
use crate::models::page::{self, Entity as Page};
use crate::services::permission::{CheckPermissionContext, PermissionService};
use crate::services::{PageRevisionService, SiteService, TextService};
use crate::types::{Action, Permission, Reference, Resource};
use ftml::data::PageRef;
use ftml::includes::{FetchedPage, IncludeRef, Includer, parse_includes};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

const MAX_INCLUDE_DEPTH: usize = 16;
const MAX_INCLUDE_DIRECTIVES: usize = 4096;
const MAX_EXPANDED_BYTES: usize = 4 * 1024 * 1024;

struct FetchedIncludes(HashMap<PageRef, Option<String>>);

impl<'t> Includer<'t> for &FetchedIncludes {
    type Error = ExnError;

    fn include_pages(
        &mut self,
        includes: &[IncludeRef<'t>],
    ) -> Result<Vec<FetchedPage<'t>>> {
        includes
            .iter()
            .map(|include| {
                let reference = include.page_ref();
                let content = self.0.get(reference).ok_or_else(|| {
                    Error::new(
                        "include resolver omitted a requested page",
                        ErrorType::Render,
                    )
                })?;
                Ok(FetchedPage {
                    page_ref: reference.clone(),
                    content: content.clone().map(Cow::Owned),
                })
            })
            .collect()
    }

    fn no_such_include(&mut self, _: &PageRef) -> Result<Cow<'t, str>> {
        Ok(Cow::Borrowed(
            "[[div class=\"error-block\"]]\nIncluded page unavailable.\n[[/div]]",
        ))
    }
}

pub(super) async fn expand_includes(
    ctx: &ServiceContext<'_>,
    mut source: String,
    site_slug: &str,
    settings: &WikitextSettings,
    viewer_id: Option<i64>,
) -> Result<(String, Vec<PageRef>)> {
    if !settings.enable_page_syntax || parse_includes(&source).is_empty() {
        return Ok((source, Vec::new()));
    }
    let site = SiteService::get(ctx, Reference::Slug(site_slug.into())).await?;
    let mut fetched = FetchedIncludes(HashMap::new());
    let mut dependencies = HashSet::new();
    let mut directive_count = 0;
    let mut depth = 0;
    loop {
        let references = collect_references(&source);
        if references.is_empty() {
            return Ok((source, dependencies.into_iter().collect()));
        }
        directive_count += references.len();
        check_work_limits(depth, directive_count, source.len())?;
        fetch_include_sources(
            ctx,
            site.site_id,
            site_slug,
            &references,
            &mut fetched,
            viewer_id,
        )
        .await?;
        let (expanded, pages) = substitute_fetched_sources(&source, settings, &fetched)?;
        dependencies.extend(pages.into_iter().filter(|page| is_local(page, site_slug)));
        source = expanded;
        depth += 1;
    }
}

fn collect_references(source: &str) -> Vec<PageRef> {
    parse_includes(source)
        .into_iter()
        .map(|(_, include)| include.page_ref().clone())
        .collect()
}

fn is_local(reference: &PageRef, site_slug: &str) -> bool {
    reference.site().is_none_or(|site| site == site_slug)
}

fn check_work_limits(depth: usize, directives: usize, source_bytes: usize) -> Result<()> {
    if depth >= MAX_INCLUDE_DEPTH {
        return Err(Error::new(
            "include expansion exceeds its nesting limit",
            ErrorType::Render,
        )
        .into());
    }
    if directives > MAX_INCLUDE_DIRECTIVES || source_bytes > MAX_EXPANDED_BYTES {
        return Err(Error::new(
            "include expansion exceeds its work limit",
            ErrorType::Render,
        )
        .into());
    }
    Ok(())
}

fn substitute_fetched_sources(
    source: &str,
    settings: &WikitextSettings,
    fetched: &FetchedIncludes,
) -> Result<(String, Vec<PageRef>)> {
    let expanded = ftml::include(source, settings, fetched, || {
        Error::new(
            "include resolver returned inconsistent pages",
            ErrorType::Render,
        )
        .into()
    })?;
    if expanded.0.len() > MAX_EXPANDED_BYTES {
        return Err(Error::new(
            "include expansion exceeds its size limit",
            ErrorType::Render,
        )
        .into());
    }
    Ok(expanded)
}

async fn fetch_include_sources(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    site_slug: &str,
    references: &[PageRef],
    fetched: &mut FetchedIncludes,
    viewer_id: Option<i64>,
) -> Result<()> {
    let pending: HashSet<_> = references
        .iter()
        .filter(|reference| !fetched.0.contains_key(*reference))
        .cloned()
        .collect();
    if pending.is_empty() {
        return Ok(());
    }
    let local_slugs: Vec<_> = pending
        .iter()
        .filter(|reference| is_local(reference, site_slug))
        .map(|reference| reference.page.clone())
        .collect();
    let pages = Page::find()
        .filter(page::Column::SiteId.eq(site_id))
        .filter(page::Column::Slug.is_in(local_slugs))
        .filter(page::Column::DeletedAt.is_null())
        .all(ctx.transaction())
        .await
        .or_raise(|| Error::new("failed to fetch included pages", ErrorType::Render))?;
    let pages: HashMap<_, _> = pages
        .into_iter()
        .map(|page| (page.slug.clone(), page))
        .collect();
    for reference in pending {
        let content = if !is_local(&reference, site_slug) {
            None
        } else if let Some(page) = pages.get(reference.page()) {
            fetch_render_source(ctx, page, viewer_id).await?
        } else {
            None
        };
        fetched.0.insert(reference, content);
    }
    Ok(())
}

pub(super) async fn fetch_render_source(
    ctx: &ServiceContext<'_>,
    page: &page::Model,
    viewer_id: Option<i64>,
) -> Result<Option<String>> {
    if !can_view_render(
        ctx,
        page.site_id,
        page.page_id,
        page.page_category_id,
        viewer_id,
    )
    .await?
    {
        return Ok(None);
    }
    let revision =
        PageRevisionService::get_latest(ctx, page.site_id, page.page_id).await?;
    // Show-to regions are revealed only in the rendered page's own source.
    TextService::get(ctx, &revision.wikitext_hash)
        .await
        .map(|source| Some(super::show_to::strip_show_to_regions(source)))
}

/// Latest same-site source visible to the render's viewer; `None` means anonymous.
pub(super) async fn fetch_render_source_by_slug(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    slug: &str,
    viewer_id: Option<i64>,
) -> Result<Option<String>> {
    let page = Page::find()
        .filter(page::Column::SiteId.eq(site_id))
        .filter(page::Column::Slug.eq(slug))
        .filter(page::Column::DeletedAt.is_null())
        .one(ctx.transaction())
        .await
        .or_raise(|| Error::new("failed to fetch shared page", ErrorType::Render))?;
    match page {
        Some(page) => fetch_render_source(ctx, &page, viewer_id).await,
        None => Ok(None),
    }
}

/// Stored renders pass no viewer; only response-only renders may pass a user ID.
pub(super) async fn can_view_render(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    page_id: i64,
    category_id: i64,
    viewer_id: Option<i64>,
) -> Result<bool> {
    check_render_view(ctx, site_id, Some(page_id), category_id, viewer_id).await
}

/// Category-wide view access, excluding page-author grants.
pub(super) async fn can_view_category_render(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    category_id: i64,
    viewer_id: Option<i64>,
) -> Result<bool> {
    check_render_view(ctx, site_id, None, category_id, viewer_id).await
}

async fn check_render_view(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    page_id: Option<i64>,
    category_id: i64,
    viewer_id: Option<i64>,
) -> Result<bool> {
    PermissionService::check_user_can(
        ctx,
        &CheckPermissionContext {
            user_id: viewer_id,
            site_id,
            page_reference: page_id.map(Reference::Id),
        },
        Permission {
            resource_type: Resource::Page,
            resource_category: Some(Reference::Id(category_id)),
            action: Action::View,
        },
    )
    .await
}
