//! Admin-only site-wide tag management; actor and site come from request context.
use super::prelude::*;
use crate::services::site_tag_admin::{
    AddTagPreview, SiteTagAdminService, TagChangeResult, TagOperation, TagPreview,
};

pub async fn site_tag_add_preview(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<AddTagPreview> {
    SiteTagAdminService::add_preview(ctx, parse!(params, BadRequest)).await
}

pub async fn site_tag_add_apply(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<TagChangeResult> {
    SiteTagAdminService::add_apply(ctx, parse!(params, BadRequest)).await
}

pub async fn site_tag_preview(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<TagPreview> {
    SiteTagAdminService::preview(ctx, parse!(params, BadRequest)).await
}

pub async fn site_tag_apply(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<TagChangeResult> {
    SiteTagAdminService::apply(ctx, parse!(params, BadRequest)).await
}

pub async fn site_tag_history(
    ctx: &ServiceContext<'_>,
    _params: Params<'static>,
) -> Result<Vec<TagOperation>> {
    SiteTagAdminService::history(ctx).await
}

pub async fn site_tag_undo(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<TagChangeResult> {
    SiteTagAdminService::undo(ctx, parse!(params, BadRequest)).await
}
