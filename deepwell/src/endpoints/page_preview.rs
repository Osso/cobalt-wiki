//! Read-only rendering of an editor's submitted page body.

use super::page::form_edit::load_updated_source;
use super::prelude::*;
use crate::models::page::Model as PageModel;
use crate::services::page::check_last_revision;
use crate::services::permission::CheckPermissionContext;
use crate::services::score::{ScoreService, ScoreValue};
use crate::types::{Action, Maybe, Reference};
use crate::utils::split_category;
use ftml::data::PageInfo;
use ftml::layout::Layout;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use wikidot_forms::Mapping;

#[derive(Debug, Deserialize)]
struct PreviewRequest {
    #[serde(default)]
    title: Maybe<String>,
    #[serde(default)]
    alt_title: Maybe<Option<String>>,
    #[serde(default)]
    tags: Maybe<Vec<String>>,
    #[serde(default)]
    wikitext: Maybe<String>,
    #[serde(default)]
    form_updates: Maybe<Mapping>,
    last_revision_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PagePreviewOutput {
    pub html: String,
}

struct PreviewMetadata {
    slug: String,
    title: String,
    alt_title: Option<String>,
    tags: Vec<String>,
    score: ScoreValue,
    layout: Layout,
}

impl PreviewRequest {
    fn validate_modes(&self, exists: bool) -> Result<()> {
        if self.wikitext.is_set() == self.form_updates.is_set() {
            return Err(Error::new(
                "exactly one of wikitext or form_updates is required",
                ErrorType::BadRequest,
            )
            .into());
        }
        if self.form_updates.is_set() && (!exists || self.last_revision_id.is_none()) {
            return Err(Error::new(
                "form_updates requires an existing page and last_revision_id",
                ErrorType::BadRequest,
            )
            .into());
        }
        if !exists && self.title.is_unset() {
            return Err(Error::new(
                "title is required for a new page",
                ErrorType::BadRequest,
            )
            .into());
        }
        Ok(())
    }
}

pub async fn page_preview(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<PagePreviewOutput> {
    let request = ctx.request();
    let site_id = request.site_id()?;
    let reference = request.page_reference()?.clone();
    let page = PageService::get_optional(ctx, site_id, reference.clone()).await?;
    authorize_preview(ctx, site_id, &reference, page.is_some()).await?;

    let input: PreviewRequest = parse!(params, Page);
    input.validate_modes(page.is_some())?;
    let site = SiteService::get(ctx, Reference::Id(site_id)).await?;
    let metadata =
        read_preview_metadata(ctx, site_id, &reference, page.as_ref(), &input).await?;
    let source = read_preview_source(ctx, site_id, reference, &input).await?;
    let html =
        render_preview(ctx, source, &site.slug, &site.locale, metadata, &input).await?;
    Ok(PagePreviewOutput { html })
}

async fn authorize_preview(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    reference: &Reference<'_>,
    exists: bool,
) -> Result<()> {
    let request = ctx.request();
    if exists {
        for action in [Action::View, Action::Edit] {
            let allowed = PageService::check_user_permission(
                ctx,
                &CheckPermissionContext {
                    user_id: request.user_id,
                    site_id,
                    page_reference: Some(reference.clone()),
                },
                action,
            )
            .await?;
            if !allowed {
                return Err(Error::new(
                    "page preview access denied",
                    ErrorType::PermissionDenied,
                )
                .into());
            }
        }
    } else {
        let allowed = match (reference, request.user_id) {
            (Reference::Slug(slug), Some(user_id)) => {
                PageService::can_create(ctx, site_id, user_id, slug).await?
            }
            _ => false,
        };
        if !allowed {
            return Err(Error::new(
                "page preview create access denied",
                ErrorType::PermissionDenied,
            )
            .into());
        }
    }
    Ok(())
}

async fn read_preview_metadata(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    reference: &Reference<'_>,
    page: Option<&PageModel>,
    input: &PreviewRequest,
) -> Result<PreviewMetadata> {
    match page {
        Some(page) => {
            let revision =
                PageRevisionService::get_latest(ctx, site_id, page.page_id).await?;
            if let Some(last_revision_id) = input.last_revision_id {
                check_last_revision(None, page.latest_revision_id, last_revision_id)?;
                check_last_revision(None, Some(revision.revision_id), last_revision_id)?;
            }
            Ok(PreviewMetadata {
                slug: page.slug.clone(),
                title: revision.title,
                alt_title: revision.alt_title,
                tags: revision.tags,
                score: ScoreService::score(ctx, page.page_id).await?,
                layout: SettingsService::get_layout(ctx, site_id, Some(page.page_id))
                    .await?,
            })
        }
        None => {
            let Reference::Slug(slug) = reference else {
                unreachable!("can_create only accepts a slug")
            };
            Ok(PreviewMetadata {
                slug: slug.to_string(),
                title: String::new(),
                alt_title: None,
                tags: Vec::new(),
                score: ScoreValue::Integer(0),
                layout: SettingsService::get_layout(ctx, site_id, None).await?,
            })
        }
    }
}

async fn read_preview_source(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    reference: Reference<'_>,
    input: &PreviewRequest,
) -> Result<String> {
    match (&input.wikitext, &input.form_updates) {
        (Maybe::Set(source), Maybe::Unset) => Ok(source.clone()),
        (Maybe::Unset, Maybe::Set(updates)) => {
            load_updated_source(
                ctx,
                site_id,
                reference,
                input.last_revision_id.expect("validated"),
                updates,
            )
            .await
        }
        _ => unreachable!("validated source mode"),
    }
}

async fn render_preview(
    ctx: &ServiceContext<'_>,
    source: String,
    site_slug: &str,
    locale: &str,
    metadata: PreviewMetadata,
    input: &PreviewRequest,
) -> Result<String> {
    let title = input.title.to_option().unwrap_or(&metadata.title);
    let alt_title = input.alt_title.to_option().unwrap_or(&metadata.alt_title);
    let tags = input.tags.to_option().unwrap_or(&metadata.tags);
    let (category, name) = split_category(&metadata.slug);
    let page_info = PageInfo {
        page: Cow::Borrowed(name),
        category: category.map(Cow::Borrowed),
        site: Cow::Borrowed(site_slug),
        title: Cow::Borrowed(title),
        alt_title: alt_title.as_deref().map(Cow::Borrowed),
        score: metadata.score,
        tags: tags.iter().map(|tag| Cow::Borrowed(tag.as_str())).collect(),
        language: Cow::Borrowed(locale),
    };
    RenderService::render_page_view(ctx, source, &page_info, metadata.layout, 1).await
}
