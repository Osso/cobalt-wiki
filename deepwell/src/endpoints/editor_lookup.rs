//! Images attached to the current editor page.

use super::prelude::*;
use crate::services::permission::CheckPermissionContext;
use crate::types::{Action, FileOrder};
use futures::future::try_join_all;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct EditorAttachment {
    pub name: String,
}

pub async fn editor_attachments(
    ctx: &ServiceContext<'_>,
    _params: Params<'static>,
) -> Result<Vec<EditorAttachment>> {
    let (site_id, page_id) = authorize_editor_page(ctx).await?;
    let files =
        FileService::get_all(ctx, site_id, page_id, Some(false), FileOrder::default())
            .await?;
    let attachments = files.into_iter().map(|file| async move {
        let revision =
            FileRevisionService::get_latest(ctx, site_id, page_id, file.file_id).await?;
        Ok::<_, exn::Exn<Error>>(
            revision
                .mime
                .starts_with("image/")
                .then_some(EditorAttachment { name: file.name }),
        )
    });
    Ok(try_join_all(attachments)
        .await?
        .into_iter()
        .flatten()
        .collect())
}

async fn authorize_editor_page(ctx: &ServiceContext<'_>) -> Result<(i64, i64)> {
    let request = ctx.request();
    let site_id = request.site_id()?;
    let reference = request.page_reference()?.clone();
    let page = PageService::get(ctx, site_id, reference.clone()).await?;
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
                "editor attachment access denied",
                ErrorType::PermissionDenied,
            )
            .into());
        }
    }
    Ok((site_id, page.page_id))
}
