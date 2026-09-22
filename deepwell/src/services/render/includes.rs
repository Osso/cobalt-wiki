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
) -> Result<(String, Vec<PageRef>)> {
    if !settings.enable_page_syntax || parse_includes(&source).is_empty() {
        return Ok((source, Vec::new()));
    }
    let site = SiteService::get(ctx, Reference::Slug(site_slug.into())).await?;
    let mut fetched = FetchedIncludes(HashMap::new());
    let mut dependencies = HashSet::new();
    let mut directive_count = 0;
    for _ in 0..MAX_INCLUDE_DEPTH {
        let references: Vec<_> = parse_includes(&source)
            .into_iter()
            .map(|(_, include)| include.page_ref().clone())
            .collect();
        if references.is_empty() {
            return Ok((source, dependencies.into_iter().collect()));
        }
        directive_count += references.len();
        if directive_count > MAX_INCLUDE_DIRECTIVES || source.len() > MAX_EXPANDED_BYTES {
            return Err(Error::new(
                "include expansion exceeds its work limit",
                ErrorType::Render,
            )
            .into());
        }
        fetch_include_sources(ctx, site.site_id, site_slug, &references, &mut fetched)
            .await?;
        let (expanded, pages) = ftml::include(&source, settings, &fetched, || {
            Error::new(
                "include resolver returned inconsistent pages",
                ErrorType::Render,
            )
            .into()
        })?;
        if expanded.len() > MAX_EXPANDED_BYTES {
            return Err(Error::new(
                "include expansion exceeds its size limit",
                ErrorType::Render,
            )
            .into());
        }
        dependencies.extend(pages);
        source = expanded;
    }
    Err(Error::new(
        "include expansion exceeds its nesting limit",
        ErrorType::Render,
    )
    .into())
}

async fn fetch_include_sources(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    site_slug: &str,
    references: &[PageRef],
    fetched: &mut FetchedIncludes,
) -> Result<()> {
    let pending: HashSet<_> = references
        .iter()
        .filter(|reference| !fetched.0.contains_key(*reference))
        .cloned()
        .collect();
    let local_slugs: Vec<_> = pending
        .iter()
        .filter(|reference| reference.site().is_none_or(|site| site == site_slug))
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
        let content = if reference.site().is_some_and(|site| site != site_slug) {
            None
        } else if let Some(page) = pages.get(reference.page()) {
            fetch_shared_source(ctx, page).await?
        } else {
            None
        };
        fetched.0.insert(reference, content);
    }
    Ok(())
}

async fn fetch_shared_source(
    ctx: &ServiceContext<'_>,
    page: &page::Model,
) -> Result<Option<String>> {
    // Compiled HTML is shared: never materialize a privileged viewer's private source.
    let can_view = PermissionService::check_user_can(
        ctx,
        &CheckPermissionContext {
            user_id: None,
            site_id: page.site_id,
            page_reference: Some(Reference::Id(page.page_id)),
        },
        Permission {
            resource_type: Resource::Page,
            resource_category: Some(Reference::Id(page.page_category_id)),
            action: Action::View,
        },
    )
    .await?;
    if !can_view {
        return Ok(None);
    }
    let revision =
        PageRevisionService::get_latest(ctx, page.site_id, page.page_id).await?;
    TextService::get(ctx, &revision.wikitext_hash)
        .await
        .map(Some)
}
