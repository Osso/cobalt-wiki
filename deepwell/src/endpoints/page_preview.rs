//! Read-only rendering of an editor's submitted page body.

use super::page::form_edit::load_updated_source;
use super::prelude::*;
use crate::services::page::check_last_revision;
use crate::services::permission::CheckPermissionContext;
use crate::services::score::{ScoreService, ScoreValue};
use crate::types::{Action, Maybe, Reference};
use crate::utils::split_category;
use ftml::data::PageInfo;
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

    if page.is_some() {
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
        let allowed = match (&reference, request.user_id) {
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

    let input: PreviewRequest = parse!(params, Page);
    input.validate_modes(page.is_some())?;
    let site = SiteService::get(ctx, Reference::Id(site_id)).await?;
    let (slug, stored_title, stored_alt_title, stored_tags, score, layout) = match &page {
        Some(page) => {
            let revision =
                PageRevisionService::get_latest(ctx, site_id, page.page_id).await?;
            if let Some(last_revision_id) = input.last_revision_id {
                check_last_revision(None, page.latest_revision_id, last_revision_id)?;
                check_last_revision(None, Some(revision.revision_id), last_revision_id)?;
            }
            (
                page.slug.clone(),
                revision.title,
                revision.alt_title,
                revision.tags,
                ScoreService::score(ctx, page.page_id).await?,
                SettingsService::get_layout(ctx, site_id, Some(page.page_id)).await?,
            )
        }
        None => {
            let Reference::Slug(slug) = reference.clone() else {
                unreachable!("can_create only accepts a slug")
            };
            (
                slug.into_owned(),
                String::new(),
                None,
                Vec::new(),
                ScoreValue::Integer(0),
                SettingsService::get_layout(ctx, site_id, None).await?,
            )
        }
    };
    let title = input.title.to_option().unwrap_or(&stored_title);
    let alt_title = input.alt_title.to_option().unwrap_or(&stored_alt_title);
    let tags = input.tags.to_option().unwrap_or(&stored_tags);
    let source = match (input.wikitext, input.form_updates) {
        (Maybe::Set(source), Maybe::Unset) => source,
        (Maybe::Unset, Maybe::Set(updates)) => {
            load_updated_source(
                ctx,
                site_id,
                reference,
                input.last_revision_id.expect("validated"),
                &updates,
            )
            .await?
        }
        _ => unreachable!("validated source mode"),
    };
    let (category, name) = split_category(&slug);
    let page_info = PageInfo {
        page: Cow::Borrowed(name),
        category: category.map(Cow::Borrowed),
        site: Cow::Borrowed(&site.slug),
        title: Cow::Borrowed(title),
        alt_title: alt_title.as_deref().map(Cow::Borrowed),
        score,
        tags: tags.iter().map(|tag| Cow::Borrowed(tag.as_str())).collect(),
        language: Cow::Borrowed(&site.locale),
    };
    let html =
        RenderService::render_page_view(ctx, source, &page_info, layout, 1).await?;
    Ok(PagePreviewOutput { html })
}
