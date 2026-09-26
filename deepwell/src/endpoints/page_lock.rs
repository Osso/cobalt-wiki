/*
 * endpoints/page_lock.rs
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
use crate::models::page_lock::Model as PageLockModel;
use crate::services::member_admin::MemberAdminService;
use crate::services::page_lock::{
    CreatePageLockInput, GetPageLockHistoryInput, RemovePageLockInput,
};
use crate::services::permission::{CheckPermissionContext, PermissionService};
use crate::services::{PageLockService, PageService};
use crate::types::{Action, Permission, Reference, Resource};
use std::net::IpAddr;

#[derive(Deserialize)]
struct SetPageBlock {
    page: Reference<'static>,
    blocked: bool,
    ip_address: IpAddr,
}

#[derive(Serialize, Clone, Debug)]
pub struct PageBlockState {
    blocked: bool,
    can_manage: bool,
}

pub async fn page_block_get(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<PageBlockState> {
    let input: GetPageLockHistoryInput = parse!(params, PageLock);
    let site_id = ctx.request().site_id()?;
    let page = PageService::get(ctx, site_id, input.page.clone()).await?;
    let can_view = PermissionService::check_user_can(
        ctx,
        &CheckPermissionContext {
            user_id: ctx.request().user_id,
            site_id,
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
        bail!(Error::new("page view denied", ErrorType::PermissionDenied));
    }
    let can_manage = match ctx.request().user_id {
        Some(user_id) => {
            MemberAdminService::is_site_page_moderator_or_admin(ctx, site_id, user_id)
                .await?
        }
        None => false,
    };
    Ok(PageBlockState {
        blocked: PageLockService::is_wikidot_blocked(ctx, input.page).await?,
        can_manage,
    })
}

pub async fn page_block_set(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<()> {
    let input: SetPageBlock = parse!(params, PageLock);
    PageLockService::set_wikidot_block(ctx, input.page, input.blocked, input.ip_address)
        .await
}

pub async fn page_lock_create(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<()> {
    let input: CreatePageLockInput = parse!(params, PageLock);

    let request = ctx.request();
    let site_id = request
        .site_id()
        .or_raise(|| Error::new("no site ID found", ErrorType::PageLock))?;
    let user_id = request
        .user_id()
        .or_raise(|| Error::new("no user ID found", ErrorType::PageLock))?;
    let page_ref = input.page.clone();

    info!(
        "Creating page lock of type {:?} for page {:?} in site {}",
        input.lock_type, page_ref, site_id,
    );

    PageLockService::create(ctx, site_id, user_id, page_ref.borrow(), input)
        .await
        .or_raise(|| Error::new("failed to create page lock", ErrorType::PageLock))?;

    Ok(())
}

pub async fn page_lock_remove(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<()> {
    let input: RemovePageLockInput = parse!(params, PageLock);

    let request = ctx.request();
    let site_id = request
        .site_id()
        .or_raise(|| Error::new("no site ID found", ErrorType::PageLock))?;
    let user_id = request
        .user_id()
        .or_raise(|| Error::new("no user ID found", ErrorType::PageLock))?;
    let page_ref = input.page.clone();

    info!(
        "Removing active page lock for page {:?} in site {}",
        page_ref, site_id,
    );

    PageLockService::remove(ctx, site_id, user_id, page_ref.borrow(), input.ip_address)
        .await
        .or_raise(|| Error::new("failed to remove page lock", ErrorType::PageLock))?;

    Ok(())
}

pub async fn page_lock_get_history(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<Vec<PageLockModel>> {
    let input: GetPageLockHistoryInput = parse!(params, PageLock);

    let request = ctx.request();
    let site_id = request
        .site_id()
        .or_raise(|| Error::new("no site ID found", ErrorType::PageLock))?;

    info!(
        "Fetching lock history for page {:?} in site {}",
        input.page, site_id,
    );

    PageLockService::get_locks_for_page(ctx, site_id, input.page.borrow())
        .await
        .or_raise(|| Error::new("failed to fetch page lock history", ErrorType::PageLock))
}
