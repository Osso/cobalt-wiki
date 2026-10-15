//! Character page rename; actor and site come from request context.
use super::prelude::*;
use crate::services::character_rename::{
    CharacterRenamePreview, CharacterRenameService, CharacterRenameStarted,
    CharacterRenameStatus,
};

pub async fn character_rename_preview(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<CharacterRenamePreview> {
    CharacterRenameService::preview(ctx, parse!(params, BadRequest)).await
}

pub async fn character_rename_start(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<CharacterRenameStarted> {
    CharacterRenameService::start(ctx, parse!(params, BadRequest)).await
}

pub async fn character_rename_status(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<CharacterRenameStatus> {
    CharacterRenameService::status(ctx, parse!(params, BadRequest)).await
}
