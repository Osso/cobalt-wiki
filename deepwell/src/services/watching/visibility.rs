//! Current recipient visibility of exact stored page revisions for notifications.

use crate::error::prelude::*;
use crate::models::page_revision::Model as PageRevision;
use crate::models::site::Model as SiteModel;
use crate::services::permission::{CheckPermissionContext, PermissionService};
use crate::services::score::ScoreValue;
use crate::services::search::plain_body;
use crate::services::{
    PageRevisionService, PageService, RenderService, ScoreService, ServiceContext,
    SettingsService, SiteService, TextService, UserService,
};
use crate::types::{Action, Permission, Reference, Resource, UserType};
use ftml::data::PageInfo;
use ftml::layout::Layout;
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleChange {
    pub title: String,
    pub slug: String,
    pub before: Option<String>,
    pub after: String,
}

/// Build recipient-visible plain text from exact stored revisions. No prior
/// permission snapshot exists: current access is required for both sources.
pub async fn visible_change(
    ctx: &ServiceContext<'_>,
    user_id: i64,
    site_id: i64,
    page_id: i64,
    previous_revision_id: Option<i64>,
    new_revision_id: i64,
) -> crate::error::Result<Option<VisibleChange>> {
    let Some(user) = UserService::get_real_optional(ctx, Reference::Id(user_id)).await?
    else {
        return Ok(None);
    };
    if user.deleted_at.is_some() || user.user_type != UserType::Regular {
        return Ok(None);
    }
    let Some(page) = PageService::get_direct_optional(ctx, page_id, false).await? else {
        return Ok(None);
    };
    if page.site_id != site_id {
        return Ok(None);
    }
    let permission_ctx = CheckPermissionContext {
        user_id: Some(user_id),
        site_id,
        page_reference: Some(Reference::Id(page_id)),
    };
    let [site_allowed, page_allowed] = PermissionService::batch_check_user_can(
        ctx,
        &permission_ctx,
        [
            Permission {
                resource_type: Resource::Site,
                resource_category: None,
                action: Action::View,
            },
            Permission {
                resource_type: Resource::Page,
                resource_category: Some(Reference::Id(page.page_category_id)),
                action: Action::View,
            },
        ],
    )
    .await?;
    if !site_allowed || !page_allowed {
        return Ok(None);
    }

    let Some(new_revision) =
        load_visible_revision(ctx, site_id, page_id, new_revision_id).await?
    else {
        return Ok(None);
    };
    let previous = match previous_revision_id {
        Some(id) => {
            let Some(revision) = load_visible_revision(ctx, site_id, page_id, id).await?
            else {
                return Ok(None);
            };
            Some(revision)
        }
        None => None,
    };
    let site = SiteService::get(ctx, Reference::Id(site_id)).await?;
    let layout = SettingsService::get_layout(ctx, site_id, Some(page_id)).await?;
    let score = ScoreService::score(ctx, page_id).await?;
    let before = match previous.as_ref() {
        Some(revision) => Some(
            render_revision_for_viewer(ctx, revision, &site, &score, layout, &user.slug)
                .await?,
        ),
        None => None,
    };
    let after =
        render_revision_for_viewer(ctx, &new_revision, &site, &score, layout, &user.slug)
            .await?;
    Ok(Some(VisibleChange {
        title: new_revision.title,
        slug: new_revision.slug,
        before,
        after,
    }))
}

async fn render_revision_for_viewer(
    ctx: &ServiceContext<'_>,
    revision: &PageRevision,
    site: &SiteModel,
    score: &ScoreValue,
    layout: Layout,
    viewer: &str,
) -> Result<String> {
    let context = || {
        Error::new(
            format!(
                "failed to render visible revision ID {}",
                revision.revision_id
            ),
            ErrorType::PageRevision,
        )
    };
    let source = TextService::get(ctx, &revision.wikitext_hash)
        .await
        .or_raise(context)?;
    let (category, page_slug) = crate::utils::split_category(&revision.slug);
    let page_info = PageInfo {
        page: Cow::Borrowed(page_slug),
        category: category.map(Cow::Borrowed),
        site: Cow::Borrowed(&site.slug),
        title: Cow::Borrowed(&revision.title),
        alt_title: revision.alt_title.as_deref().map(Cow::Borrowed),
        score: score.clone(),
        tags: revision
            .tags
            .iter()
            .map(|tag| Cow::Borrowed(tag.as_str()))
            .collect(),
        language: Cow::Borrowed(&site.locale),
    };
    let html =
        RenderService::render_page_for_viewer(ctx, source, &page_info, layout, viewer)
            .await
            .or_raise(context)?;
    Ok(plain_body(&html))
}

async fn load_visible_revision(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    page_id: i64,
    revision_id: i64,
) -> crate::error::Result<Option<PageRevision>> {
    let revision = PageRevisionService::get_direct_optional(ctx, revision_id).await?;
    Ok(revision.filter(|revision| {
        revision.site_id == site_id
            && revision.page_id == page_id
            && revision.hidden.is_empty()
    }))
}
