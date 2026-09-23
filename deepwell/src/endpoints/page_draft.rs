//! Shared unpublished source per site and canonical page slug.

use super::page::form_edit::load_updated_source;
use super::prelude::*;
use crate::models::page::Model as PageModel;
use crate::models::page_draft::{self, Entity as PageDraft};
use crate::services::PageDraftService;
use crate::services::page::check_last_revision;
use crate::services::permission::CheckPermissionContext;
use crate::services::view::{extract_page_form, template_slug};
use crate::types::{Action, Reference};
use sea_orm::{ActiveValue::Set, EntityTrait};
use sea_query::OnConflict;
use serde::{Deserialize, Serialize};
use wikidot_forms::Mapping;
use wikidot_normalize::normalize;

#[derive(Debug, Clone, Serialize)]
pub struct PageDraftOutput {
    pub draft: Option<DraftContent>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DraftContent {
    pub title: String,
    pub wikitext: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: sea_orm::prelude::TimeDateTimeWithTimeZone,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_values: Option<Mapping>,
}

#[derive(Debug, Deserialize)]
struct SaveDraftRequest {
    title: String,
    wikitext: Option<String>,
    form_updates: Option<Mapping>,
    last_revision_id: Option<i64>,
}

struct DraftTarget {
    site_id: i64,
    slug: String,
    page: Option<PageModel>,
}

async fn authorize_target(ctx: &ServiceContext<'_>) -> Result<DraftTarget> {
    let request = ctx.request();
    let site_id = request.site_id()?;
    let reference = request.page_reference()?.clone();
    let mut slug = match &reference {
        Reference::Slug(slug) => slug.to_string(),
        Reference::Id(_) => String::new(),
    };
    if !slug.is_empty() {
        normalize(&mut slug);
    }
    let page = PageService::get_optional(ctx, site_id, reference.clone()).await?;
    if let Some(page) = &page {
        slug = page.slug.clone();
    }
    reject_mismatched_origin(ctx, site_id, &slug, page.as_ref()).await?;
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
                    "page draft access denied",
                    ErrorType::PermissionDenied,
                )
                .into());
            }
        }
    } else {
        let allowed = match (&reference, request.user_id) {
            (Reference::Slug(_), Some(user_id)) => {
                PageService::can_create(ctx, site_id, user_id, &slug).await?
            }
            _ => false,
        };
        if !allowed {
            return Err(Error::new(
                "page draft create access denied",
                ErrorType::PermissionDenied,
            )
            .into());
        }
    }
    Ok(DraftTarget {
        site_id,
        slug,
        page,
    })
}

async fn reject_mismatched_origin(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    slug: &str,
    page: Option<&PageModel>,
) -> Result<()> {
    let draft = PageDraft::find_by_id((site_id, slug.to_owned()))
        .one(ctx.transaction())
        .await
        .or_raise(|| Error::new("failed to check page draft origin", ErrorType::Page))?;
    if let Some(origin_page_id) = draft.and_then(|draft| draft.origin_page_id) {
        if page.map(|page| page.page_id) != Some(origin_page_id) {
            return Err(Error::new(
                "page draft's original page is no longer at this slug",
                ErrorType::PermissionDenied,
            )
            .into());
        }
    }
    Ok(())
}

async fn read_draft(
    ctx: &ServiceContext<'_>,
    target: &DraftTarget,
) -> Result<PageDraftOutput> {
    let model = PageDraft::find_by_id((target.site_id, target.slug.clone()))
        .one(ctx.transaction())
        .await
        .or_raise(|| Error::new("failed to read page draft", ErrorType::Page))?;
    let Some(model) = model else {
        return Ok(PageDraftOutput { draft: None });
    };
    let form_values = read_form_values(ctx, target, &model.wikitext).await?;
    Ok(PageDraftOutput {
        draft: Some(DraftContent {
            title: model.title,
            wikitext: model.wikitext,
            updated_at: model.updated_at,
            form_values,
        }),
    })
}

async fn read_form_values(
    ctx: &ServiceContext<'_>,
    target: &DraftTarget,
    source: &str,
) -> Result<Option<Mapping>> {
    let Some(template_slug) = template_slug(&target.slug) else {
        return Ok(None);
    };
    let template = ViewService::load_visible_template_source(
        ctx,
        target.site_id,
        ctx.request().user_id,
        &template_slug,
    )
    .await?;
    match extract_page_form(template.as_deref(), source) {
        Ok(form) => Ok(form.map(|form| form.values)),
        Err(error) => {
            warn!(
                "Cannot parse in-progress draft form for {}: {error}",
                target.slug
            );
            Ok(None)
        }
    }
}

pub async fn page_draft_get(
    ctx: &ServiceContext<'_>,
    _params: Params<'static>,
) -> Result<PageDraftOutput> {
    let target = authorize_target(ctx).await?;
    read_draft(ctx, &target).await
}

pub async fn page_draft_save(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<PageDraftOutput> {
    let target = authorize_target(ctx).await?;
    let input: SaveDraftRequest = parse!(params, Page);
    let source = resolve_source(
        ctx,
        &target,
        input.wikitext,
        input.form_updates,
        input.last_revision_id,
    )
    .await?;
    let model = page_draft::ActiveModel {
        site_id: Set(target.site_id),
        slug: Set(target.slug.clone()),
        title: Set(input.title),
        wikitext: Set(source),
        saved_by_user_id: Set(ctx.request().user_id),
        origin_page_id: Set(target.page.as_ref().map(|page| page.page_id)),
        updated_at: Set(crate::utils::now()),
    };
    PageDraft::insert(model)
        .on_conflict(
            OnConflict::columns([page_draft::Column::SiteId, page_draft::Column::Slug])
                .update_columns([
                    page_draft::Column::Title,
                    page_draft::Column::Wikitext,
                    page_draft::Column::SavedByUserId,
                    page_draft::Column::OriginPageId,
                    page_draft::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(ctx.transaction())
        .await
        .or_raise(|| Error::new("failed to save page draft", ErrorType::Page))?;
    read_draft(ctx, &target).await
}

async fn resolve_source(
    ctx: &ServiceContext<'_>,
    target: &DraftTarget,
    wikitext: Option<String>,
    form_updates: Option<Mapping>,
    last_revision_id: Option<i64>,
) -> Result<String> {
    match (wikitext, form_updates, &target.page) {
        (Some(source), None, Some(page)) => {
            check_base(page, last_revision_id)?;
            Ok(source)
        }
        (Some(source), None, None) if last_revision_id.is_none() => Ok(source),
        (None, Some(updates), Some(page)) => {
            let revision_id = check_base(page, last_revision_id)?;
            load_updated_source(ctx, target.site_id,
                Reference::Id(page.page_id), revision_id, &updates).await
        }
        _ => Err(Error::new("draft requires exactly one of wikitext or form_updates and a matching published base", ErrorType::BadRequest).into()),
    }
}

fn check_base(page: &PageModel, provided: Option<i64>) -> Result<i64> {
    let revision_id = provided.ok_or_else(|| {
        Error::new(
            "last_revision_id is required for existing page drafts",
            ErrorType::BadRequest,
        )
    })?;
    check_last_revision(None, page.latest_revision_id, revision_id)?;
    Ok(revision_id)
}

#[derive(Debug, Clone, Serialize)]
pub struct PageDraftDeleteOutput {
    pub deleted: bool,
}

pub async fn page_draft_delete(
    ctx: &ServiceContext<'_>,
    _params: Params<'static>,
) -> Result<PageDraftDeleteOutput> {
    let target = authorize_target(ctx).await?;
    PageDraftService::delete_for_target(ctx.transaction(), target.site_id, &target.slug)
        .await?;
    Ok(PageDraftDeleteOutput { deleted: true })
}
