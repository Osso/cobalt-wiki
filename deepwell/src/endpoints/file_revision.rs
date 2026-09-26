/*
 * endpoints/file_revision.rs
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
use crate::models::file_revision::Model as FileRevisionModel;
use crate::services::file::GetFile;
use crate::services::file_revision::{
    FileRevisionCountOutput, GetFileRevision, GetFileRevisionRange, UpdateFileRevision,
};
use crate::services::permission::{CheckPermissionContext, PermissionService};
use crate::types::{Action, Permission, Reference, Resource};

pub async fn file_revision_count(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<FileRevisionCountOutput> {
    let GetFile {
        site_id,
        page_id,
        file: file_reference,
    } = parse!(params, FileRevision);

    info!("Getting latest revision for file ID {page_id} in site ID {site_id}");

    let make_error = || {
        Error::new(
            "failed to get count of file revisions",
            ErrorType::FileRevision,
        )
    };

    require_history_site(ctx, site_id)?;
    let file_id = FileService::get_id(ctx, page_id, file_reference)
        .await
        .or_raise(make_error)?;
    authorize_file_history(ctx, file_id, site_id).await?;

    let revision_count = FileRevisionService::count(ctx, page_id, file_id)
        .await
        .or_raise(make_error)?;

    Ok(FileRevisionCountOutput {
        revision_count,
        first_revision: 0,
        last_revision: revision_count.get() - 1,
    })
}

pub async fn file_revision_get(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<Option<FileRevisionModel>> {
    let input: GetFileRevision = parse!(params, FileRevision);

    info!(
        "Getting file revision {} for file ID {} on page ID {}",
        input.revision_number, input.file_id, input.page_id,
    );

    authorize_file_history(ctx, input.file_id, input.site_id).await?;
    FileRevisionService::get_optional(ctx, input)
        .await
        .or_raise(|| Error::new("failed to get file revision", ErrorType::FileRevision))
}

pub async fn file_revision_range(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<Vec<FileRevisionModel>> {
    let input: GetFileRevisionRange = parse!(params, FileRevision);

    let site_id = ctx.request().site_id.ok_or_else(deny_file_history)?;
    authorize_file_history(ctx, input.file_id, site_id).await?;
    FileRevisionService::get_range(ctx, input)
        .await
        .or_raise(|| {
            Error::new(
                "failed to get range of file revisions",
                ErrorType::FileRevision,
            )
        })
}

fn deny_file_history() -> crate::error::ExnError {
    Error::new(
        "user does not have permission to read this file's history",
        ErrorType::PermissionDenied,
    )
    .into()
}

fn require_history_site(ctx: &ServiceContext<'_>, site_id: i64) -> Result<()> {
    if ctx.request().site_id != Some(site_id) {
        return Err(deny_file_history());
    }
    Ok(())
}

async fn authorize_file_history(
    ctx: &ServiceContext<'_>,
    file_id: i64,
    site_id: i64,
) -> Result<()> {
    require_history_site(ctx, site_id)?;
    let file = FileService::get_direct(ctx, file_id, true).await?;
    if file.site_id != site_id {
        return Err(deny_file_history());
    }
    let page = PageService::get(ctx, file.site_id, Reference::Id(file.page_id)).await?;
    let allowed = PermissionService::check_user_can(
        ctx,
        &CheckPermissionContext {
            user_id: ctx.request().user_id,
            site_id: file.site_id,
            page_reference: Some(Reference::Id(page.page_id)),
        },
        Permission {
            resource_type: Resource::Page,
            resource_category: Some(Reference::Id(page.page_category_id)),
            action: Action::Edit,
        },
    )
    .await?;
    if !allowed {
        return Err(deny_file_history());
    }
    Ok(())
}

pub async fn file_revision_edit(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<FileRevisionModel> {
    let input: UpdateFileRevision = parse!(params, FileRevision);

    info!(
        "Editing file revision ID {} for file ID {} on page {}",
        input.revision_id, input.file_id, input.page_id,
    );

    FileRevisionService::update(ctx, input)
        .await
        .or_raise(|| Error::new("failed to edit file revision", ErrorType::FileRevision))
}
