/*
 * services/relation/site_member.rs
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
use crate::models::relation;
use crate::services::audit::{AuditEvent, AuditService};
use std::net::IpAddr;
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "cause", content = "user_id")]
pub enum SiteMemberAccepted {
    CreatedSite,
    SelfJoined,
    Password,
    Accepted(i64),
    Invitation(i64),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SiteMemberData {
    pub accepted: SiteMemberAccepted,
}

impl_relation! {
    name => SiteMember,
    dest => site_id: Site,
    from => user_id: User,
    data => SiteMemberData,
    create_fn => private,
    remove_fn => private,
}

impl RelationService {
    pub async fn create_site_member(
        ctx: &ServiceContext<'_>,
        CreateSiteMember {
            site_id,
            user_id,
            metadata,
            created_by,
        }: CreateSiteMember,
        ip_address: IpAddr,
    ) -> Result<()> {
        let make_error = || {
            Error::new(
                format!(
                    "failed to add user ID {} as member of site ID {}, created by user ID {}",
                    user_id, site_id, created_by,
                ),
                ErrorType::SiteMemberRelation,
            )
        };

        // Cannot join if banned
        Self::check_site_ban(ctx, GetSiteBan { site_id, user_id }, "join")
            .await
            .or_raise(make_error)?;

        Self::create_site_member_inner(
            ctx,
            CreateSiteMemberInner {
                site_id,
                user_id,
                created_by,
                metadata: &metadata,
            },
        )
        .await
        .or_raise(make_error)?;

        AuditService::log(
            ctx,
            ip_address,
            AuditEvent::JoinSiteMember {
                user_id,
                site_id,
                joining_user_id: created_by,
            },
        )
        .await
        .or_raise(make_error)?;

        Ok(())
    }

    /// Backdates a membership to when the user originally joined, e.g. on Wikidot.
    pub async fn set_site_member_joined_at(
        ctx: &ServiceContext<'_>,
        input: GetSiteMember,
        joined_at: OffsetDateTime,
    ) -> Result<()> {
        let membership = Self::get_site_member(ctx, input).await?;
        relation::ActiveModel {
            relation_id: Set(membership.relation_id),
            created_at: Set(joined_at),
            ..Default::default()
        }
        .update(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new(
                "failed to set site member join time",
                ErrorType::SiteMemberRelation,
            )
        })?;
        Ok(())
    }

    pub async fn remove_site_member(
        ctx: &ServiceContext<'_>,
        RemoveSiteMember {
            site_id,
            user_id,
            removed_by,
        }: RemoveSiteMember,
        ip_address: IpAddr,
        reason: &str,
    ) -> Result<RelationModel> {
        let make_error = || {
            Error::new(
                format!(
                    "failed to remove user ID {} as member of site ID {}, removed by user ID {}",
                    user_id, site_id, removed_by,
                ),
                ErrorType::SiteMemberRelation,
            )
        };

        let model = Self::remove_site_member_inner(
            ctx,
            RemoveSiteMember {
                site_id,
                user_id,
                removed_by,
            },
        )
        .await
        .or_raise(make_error)?;

        AuditService::log(
            ctx,
            ip_address,
            AuditEvent::RemoveSiteMember {
                user_id,
                site_id,
                removing_user_id: removed_by,
                reason,
            },
        )
        .await
        .or_raise(make_error)?;

        Ok(model)
    }
}
