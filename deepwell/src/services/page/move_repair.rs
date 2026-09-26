//! Selected rename repairs use normal revisions without consuming editor drafts.

use super::move_dependencies::rewrite_move_dependencies;
use super::prelude::*;
use crate::models::page::{self, Model as PageModel};
use crate::services::audit::{AuditEvent, AuditService};
use crate::services::link::{GetPageBacklinksOutput, LinkService};
use crate::services::page_revision::{CreatePageRevision, CreatePageRevisionBody};
use crate::services::permission::{CheckPermissionContext, PermissionService};
use crate::services::{PageLockService, PageRevisionService, TextService};
use crate::types::{
    Action, ConnectionType, PageId, PageRevisionType, Permission, Resource,
};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::net::IpAddr;

pub(super) struct MoveRepair<'a> {
    pub site_id: i64,
    pub user_id: i64,
    pub ip_address: IpAddr,
    pub old_slug: &'a str,
    pub new_slug: &'a str,
    pub selected: &'a [i64],
}

pub(super) async fn repair_selected_dependencies(
    ctx: &ServiceContext<'_>,
    change: MoveRepair<'_>,
    mut remaining: GetPageBacklinksOutput,
) -> Result<(Vec<i64>, GetPageBacklinksOutput)> {
    let candidates: BTreeSet<_> = remaining
        .links
        .iter()
        .chain(&remaining.inclusions)
        .map(|page| page.page_id)
        .collect();
    let selected: BTreeSet<_> = change.selected.iter().copied().collect();
    let mut repaired = Vec::new();
    let mut leftovers = HashMap::new();
    for page_id in candidates.intersection(&selected).copied() {
        if repair_dependency(ctx, &change, page_id).await? {
            repaired.push(page_id);
            leftovers.insert(page_id, load_remaining_types(ctx, &change, page_id).await?);
        }
    }
    remaining.links.retain(|page| {
        leftovers
            .get(&page.page_id)
            .is_none_or(|types| types.contains(&ConnectionType::Link))
    });
    remaining.inclusions.retain(|page| {
        leftovers.get(&page.page_id).is_none_or(|types| {
            types.contains(&ConnectionType::IncludeMessy)
                || types.contains(&ConnectionType::IncludeElements)
        })
    });
    Ok((repaired, remaining))
}

async fn load_editable_dependency(
    ctx: &ServiceContext<'_>,
    change: &MoveRepair<'_>,
    page_id: i64,
) -> Result<Option<PageModel>> {
    let page = page::Entity::find_by_id(page_id)
        .filter(page::Column::SiteId.eq(change.site_id))
        .filter(page::Column::DeletedAt.is_null())
        .lock_exclusive()
        .one(ctx.transaction())
        .await
        .or_raise(|| Error::new("failed to lock move dependency", ErrorType::Page))?;
    let Some(page) = page else {
        return Ok(None);
    };
    let can_edit = PermissionService::check_user_can(
        ctx,
        &CheckPermissionContext {
            user_id: Some(change.user_id),
            site_id: change.site_id,
            page_reference: Some(Reference::Id(page_id)),
        },
        Permission {
            resource_type: Resource::Page,
            resource_category: Some(Reference::Id(page.page_category_id)),
            action: Action::Edit,
        },
    )
    .await?;
    if !can_edit || has_active_lock(ctx, change.site_id, page_id).await? {
        return Ok(None);
    }
    Ok(Some(page))
}

async fn has_active_lock(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    page_id: i64,
) -> Result<bool> {
    let locks =
        PageLockService::get_locks_for_page(ctx, site_id, Reference::Id(page_id)).await?;
    let current_time = now();
    Ok(locks.iter().any(|lock| {
        lock.deleted_at.is_none()
            && lock.expires_at.is_none_or(|expires| expires > current_time)
    }))
}

async fn repair_dependency(
    ctx: &ServiceContext<'_>,
    change: &MoveRepair<'_>,
    page_id: i64,
) -> Result<bool> {
    let Some(page) = load_editable_dependency(ctx, change, page_id).await? else {
        return Ok(false);
    };
    let previous = PageRevisionService::get_latest(ctx, change.site_id, page_id).await?;
    let source = TextService::get(ctx, &previous.wikitext_hash).await?;
    let rewritten = rewrite_move_dependencies(&source, change.old_slug, change.new_slug);
    if rewritten == source {
        return Ok(false);
    }
    let revision = CreatePageRevision {
        user_id: change.user_id,
        comments: format!(
            "Automatic update related to page rename: \"{}\" to \"{}\".",
            change.old_slug, change.new_slug
        ),
        revision_type: PageRevisionType::Regular,
        body: CreatePageRevisionBody {
            wikitext: Maybe::Set(rewritten),
            ..Default::default()
        },
    };
    let output = PageRevisionService::create(
        ctx,
        PageId::from_page_model(&page),
        revision,
        previous,
    )
    .await?;
    let Some(output) = output else {
        bail!(Error::new(
            "changed move dependency produced no revision",
            ErrorType::Page
        ));
    };
    persist_dependency_revision(ctx, change, page_id, output.revision_id).await?;
    Ok(true)
}

async fn persist_dependency_revision(
    ctx: &ServiceContext<'_>,
    change: &MoveRepair<'_>,
    page_id: i64,
    revision_id: i64,
) -> Result<()> {
    page::ActiveModel {
        page_id: Set(page_id),
        latest_revision_id: Set(Some(revision_id)),
        updated_at: Set(Some(now())),
        ..Default::default()
    }
    .update(ctx.transaction())
    .await
    .or_raise(|| {
        Error::new("failed to update move dependency revision", ErrorType::Page)
    })?;
    AuditService::log(
        ctx,
        change.ip_address,
        AuditEvent::PageEdit {
            site_id: change.site_id,
            page_id,
            user_id: change.user_id,
            revision_id: Some(revision_id),
        },
    )
    .await?;
    Ok(())
}

async fn load_remaining_types(
    ctx: &ServiceContext<'_>,
    change: &MoveRepair<'_>,
    page_id: i64,
) -> Result<HashSet<ConnectionType>> {
    let links = LinkService::get_from(ctx, page_id).await?;
    Ok(links
        .absent
        .into_iter()
        .filter(|link| {
            link.to_site_id == change.site_id && link.to_page_slug == change.old_slug
        })
        .map(|link| link.connection_type)
        .collect())
}
