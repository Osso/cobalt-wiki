/*
 * services/page_lock/service.rs
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

use sea_query::Cond;
use std::net::IpAddr;
use time::OffsetDateTime;

use super::prelude::*;
use crate::models::page_lock::{self, Entity as PageLock, Model as PageLockModel};
use crate::services::audit::{AuditEvent, AuditService};
use crate::services::permission::{CheckPermissionContext, PermissionService};
use crate::services::relation::GetPageAttributions;
use crate::services::{MemberAdminService, PageService, RelationService};
use crate::types::{Action, PageLockType, Permission, Reference, Resource};

#[derive(Debug, Clone)]
pub struct PageLockService;

impl PageLockService {
    pub async fn create(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        user_id: i64,
        page_ref: Reference<'_>,
        input: CreatePageLockInput,
    ) -> Result<PageLockModel> {
        let txn = ctx.transaction();

        let make_error = || {
            Error::new(
                format!("failed to create page lock for page {:?}", page_ref),
                ErrorType::PageLock,
            )
        };

        Self::require_actor(ctx, site_id, user_id)?;
        let page = PageService::get(ctx, site_id, page_ref.borrow())
            .await
            .or_raise(make_error)?;
        let page_id = page.page_id;
        if input.lock_type == PageLockType::Wikidot {
            Self::require_page_moderator(ctx, site_id, user_id).await?;
        } else {
            Self::require_page_edit(
                ctx,
                site_id,
                page_id,
                page.page_category_id,
                user_id,
            )
            .await?;
        }

        // Check if any active lock exists for the page
        let existing_lock = Self::get_active_lock_for_page(ctx, page_id)
            .await
            .or_raise(make_error)?;

        if let Some(old_lock) = existing_lock {
            Self::require_lock_bypass(
                ctx,
                site_id,
                page_id,
                page.page_category_id,
                user_id,
            )
            .await?;
            if !input.override_existing {
                bail!(Error::new(
                    format!(
                        "an active lock already exists for page {:?}, please remove it first.",
                        page_ref
                    ),
                    ErrorType::PageLockExists
                ));
            } else {
                // Soft delete the old lock
                page_lock::ActiveModel {
                    page_lock_id: Set(old_lock.page_lock_id),
                    deleted_at: Set(Some(now())),
                    updated_at: Set(Some(now())),
                    ..Default::default()
                }
                .update(txn)
                .await
                .or_raise(make_error)?;
            }
        }

        // Create the page lock
        let new_lock = page_lock::ActiveModel {
            page_id: Set(page_id),
            user_id: Set(user_id),
            lock_type: Set(input.lock_type),
            reason: Set(input.reason.unwrap_or_default()),
            expires_at: Set(input.expires_at),
            from_wikidot: Set(input.from_wikidot),
            created_at: Set(now()),
            deleted_at: Set(None),
            updated_at: Set(None),
            ..Default::default()
        }
        .insert(txn)
        .await
        .or_raise(make_error)?;

        AuditService::log(
            ctx,
            input.ip_address,
            AuditEvent::PageLockCreate {
                user_id,
                site_id,
                page_id,
                page_lock_id: new_lock.page_lock_id,
                lock_type: input.lock_type,
            },
        )
        .await
        .or_raise(make_error)?;

        Ok(new_lock)
    }

    pub async fn remove(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        user_id: i64,
        page_ref: Reference<'_>,
        ip_address: IpAddr,
    ) -> Result<Option<PageLockModel>> {
        let txn = ctx.transaction();

        let make_error = || {
            Error::new(
                format!("failed to remove page lock for page {:?}", page_ref),
                ErrorType::PageLock,
            )
        };

        Self::require_actor(ctx, site_id, user_id)?;
        let page = PageService::get(ctx, site_id, page_ref.borrow())
            .await
            .or_raise(make_error)?;
        let page_id = page.page_id;

        // Fetch the active lock to be removed
        let maybe_lock = Self::get_active_lock_for_page(ctx, page_id)
            .await
            .or_raise(make_error)?;

        // If no lock, return
        let page_lock = match maybe_lock {
            Some(lock) => lock,
            None => return Ok(None),
        };

        if page_lock.lock_type == PageLockType::Wikidot {
            Self::require_page_moderator(ctx, site_id, user_id).await?;
        } else {
            Self::require_page_edit(
                ctx,
                site_id,
                page_id,
                page.page_category_id,
                user_id,
            )
            .await?;
            Self::require_lock_bypass(
                ctx,
                site_id,
                page_id,
                page.page_category_id,
                user_id,
            )
            .await?;
        }

        // Mark the page lock as deleted
        let removed_lock = page_lock::ActiveModel {
            page_lock_id: Set(page_lock.page_lock_id),
            deleted_at: Set(Some(now())),
            updated_at: Set(Some(now())),
            ..Default::default()
        }
        .update(txn)
        .await
        .or_raise(make_error)?;

        AuditService::log(
            ctx,
            ip_address,
            AuditEvent::PageLockRemove {
                user_id,
                page_id: page_lock.page_id,
                page_lock_id: page_lock.page_lock_id,
                lock_type: page_lock.lock_type,
            },
        )
        .await
        .or_raise(make_error)?;

        Ok(Some(removed_lock))
    }

    fn require_actor(ctx: &ServiceContext<'_>, site_id: i64, user_id: i64) -> Result<()> {
        let request = ctx.request();
        if request.site_id != Some(site_id)
            || request.user_id != Some(user_id)
            || request
                .session
                .as_ref()
                .is_some_and(|session| session.restricted)
        {
            bail!(Error::new(
                "page lock actor or site mismatch",
                ErrorType::PermissionDenied
            ));
        }
        Ok(())
    }

    async fn require_page_moderator(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        user_id: i64,
    ) -> Result<()> {
        if !MemberAdminService::is_site_page_moderator_or_admin(ctx, site_id, user_id)
            .await?
        {
            bail!(Error::new(
                "only site page moderators and admins can change Wikidot blocks",
                ErrorType::PermissionDenied
            ));
        }
        Ok(())
    }

    async fn has_page_permission(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        page_id: i64,
        category_id: i64,
        user_id: i64,
        action: Action,
    ) -> Result<bool> {
        PermissionService::check_user_can(
            ctx,
            &CheckPermissionContext {
                user_id: Some(user_id),
                site_id,
                page_reference: Some(Reference::Id(page_id)),
            },
            Permission {
                resource_type: Resource::Page,
                resource_category: Some(Reference::Id(category_id)),
                action,
            },
        )
        .await
    }

    async fn require_page_edit(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        page_id: i64,
        category_id: i64,
        user_id: i64,
    ) -> Result<()> {
        if !Self::has_page_permission(
            ctx,
            site_id,
            page_id,
            category_id,
            user_id,
            Action::Edit,
        )
        .await?
        {
            bail!(Error::new(
                "page edit permission required to manage native locks",
                ErrorType::PermissionDenied
            ));
        }
        Ok(())
    }

    async fn require_lock_bypass(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        page_id: i64,
        category_id: i64,
        user_id: i64,
    ) -> Result<()> {
        let bypass =
            Self::can_user_bypass_lock(ctx, site_id, page_id, Some(category_id), user_id)
                .await?;
        if !bypass.can_edit {
            bail!(Error::new(
                "current page lock cannot be bypassed",
                ErrorType::PermissionDenied
            ));
        }
        Ok(())
    }

    /// Whether the page currently has a legacy Wikidot block.
    pub async fn is_wikidot_blocked(
        ctx: &ServiceContext<'_>,
        page_ref: Reference<'_>,
    ) -> Result<bool> {
        let site_id = ctx.request().site_id()?;
        let page_id = PageService::get(ctx, site_id, page_ref).await?.page_id;
        Ok(Self::get_active_lock_for_page(ctx, page_id)
            .await?
            .is_some_and(|lock| lock.lock_type == PageLockType::Wikidot))
    }

    /// Set the legacy block without replacing a different native lock type.
    pub async fn set_wikidot_block(
        ctx: &ServiceContext<'_>,
        page_ref: Reference<'_>,
        blocked: bool,
        ip_address: IpAddr,
    ) -> Result<()> {
        let site_id = ctx.request().site_id()?;
        let user_id = ctx.request().user_id.ok_or_else(|| {
            Error::new(
                "page block requires a signed-in actor",
                ErrorType::PermissionDenied,
            )
        })?;
        Self::require_actor(ctx, site_id, user_id)?;
        Self::require_page_moderator(ctx, site_id, user_id).await?;
        let page_id = PageService::get(ctx, site_id, page_ref.borrow())
            .await?
            .page_id;
        let active = Self::get_active_lock_for_page(ctx, page_id).await?;
        match (blocked, active) {
            (true, None) => {
                Self::create(
                    ctx,
                    site_id,
                    user_id,
                    Reference::Id(page_id),
                    CreatePageLockInput {
                        page: Reference::Id(page_id),
                        expires_at: None,
                        from_wikidot: false,
                        lock_type: PageLockType::Wikidot,
                        reason: None,
                        override_existing: false,
                        ip_address,
                    },
                )
                .await?;
            }
            (true, Some(lock)) if lock.lock_type != PageLockType::Wikidot => {
                bail!(Error::new(
                    "a native page lock already exists",
                    ErrorType::PageLockExists
                ));
            }
            (false, Some(lock)) if lock.lock_type == PageLockType::Wikidot => {
                Self::remove(ctx, site_id, user_id, Reference::Id(page_id), ip_address)
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn get_locks_for_page(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        page_ref: Reference<'_>,
    ) -> Result<Vec<PageLockModel>> {
        let txn = ctx.transaction();

        let make_error = || {
            Error::new(
                format!("failed to fetch active lock for page {:?}", page_ref),
                ErrorType::PageLock,
            )
        };

        // Fetch the page to get its ID
        let page_id = PageService::get_id(ctx, site_id, page_ref.borrow())
            .await
            .or_raise(make_error)?;

        // Fetch all historical locks for the page, including expired and deleted ones
        let locks = PageLock::find()
            .filter(page_lock::Column::PageId.eq(page_id))
            .order_by_desc(page_lock::Column::CreatedAt)
            .all(txn)
            .await
            .or_raise(make_error)?;

        Ok(locks)
    }

    async fn get_active_lock_for_page(
        ctx: &ServiceContext<'_>,
        page_id: i64,
    ) -> Result<Option<PageLockModel>> {
        let txn = ctx.transaction();

        let make_error = || {
            Error::new(
                format!("failed to fetch active lock for page ID {}", page_id),
                ErrorType::PageLock,
            )
        };

        // Fetch the active lock for the page
        let active_lock = PageLock::find()
            .filter(
                Condition::all()
                    .add(page_lock::Column::PageId.eq(page_id))
                    .add(page_lock::Column::DeletedAt.is_null())
                    .add(
                        Condition::any()
                            .add(page_lock::Column::ExpiresAt.gt(now()))
                            .add(page_lock::Column::ExpiresAt.is_null()),
                    ),
            )
            .one(txn)
            .await
            .or_raise(make_error)?;

        Ok(active_lock)
    }

    pub async fn can_user_bypass_lock(
        ctx: &ServiceContext<'_>,
        site_id: i64,
        page_id: i64,
        _page_category_id: Option<i64>,
        user_id: i64,
    ) -> Result<CheckLockBypassOutput> {
        let make_error = || {
            Error::new(
                format!(
                    "failed to check lock bypass for page ID {} and user ID {}",
                    page_id, user_id
                ),
                ErrorType::PageLock,
            )
        };

        Self::require_actor(ctx, site_id, user_id)?;
        let page = PageService::get_direct(ctx, page_id, true)
            .await
            .or_raise(make_error)?;
        if page.site_id != site_id {
            bail!(Error::new(
                "page is not in the actor's site",
                ErrorType::PermissionDenied
            ));
        }
        let active_lock = Self::get_active_lock_for_page(ctx, page_id)
            .await
            .or_raise(make_error)?;

        if let Some(lock) = active_lock {
            let can_bypass = match lock.lock_type {
                PageLockType::Wikidot => {
                    MemberAdminService::is_site_page_moderator_or_admin(
                        ctx, site_id, user_id,
                    )
                    .await
                    .or_raise(make_error)?
                }
                PageLockType::PermissionOnly => Self::has_page_permission(
                    ctx,
                    site_id,
                    page_id,
                    page.page_category_id,
                    user_id,
                    Action::BypassLock,
                )
                .await
                .or_raise(make_error)?,
                PageLockType::AuthorOrPermissionOnly => {
                    // Check if the user is the author of the page
                    let attributions = RelationService::get_page_attributions(
                        ctx,
                        GetPageAttributions {
                            site_id,
                            page: page_id.into(),
                        },
                    )
                    .await
                    .or_raise(make_error)?;

                    // User can bypass if they are an author of this page or have bypass permission
                    let is_author =
                        attributions.iter().any(|attr| attr.user_id == user_id);
                    is_author
                        || Self::has_page_permission(
                            ctx,
                            site_id,
                            page_id,
                            page.page_category_id,
                            user_id,
                            Action::BypassLock,
                        )
                        .await
                        .or_raise(make_error)?
                }
            };
            Ok(CheckLockBypassOutput {
                lock_present: true,
                can_edit: can_bypass,
            })
        } else {
            // No active lock, so no need to bypass
            Ok(CheckLockBypassOutput {
                lock_present: false,
                can_edit: true,
            })
        }
    }
}
