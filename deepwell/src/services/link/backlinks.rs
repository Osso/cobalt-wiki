//! Read-only, viewer-filtered incoming page links and inclusions.

use crate::error::prelude::*;
use crate::models::page_connection::{self, Entity as PageConnection};
use crate::models::{page, page_revision};
use crate::services::permission::{CheckPermissionContext, PermissionService};
use crate::services::{PageService, ServiceContext};
use crate::types::{Action, ConnectionType, Permission, Reference, Resource};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Deserialize, Debug, Clone)]
pub struct GetPageBacklinks {
    pub site_id: i64,
    pub page_id: i64,
}

#[derive(Serialize, Debug, Clone)]
pub struct GetPageBacklinksOutput {
    pub links: Vec<BacklinkPage>,
    pub inclusions: Vec<BacklinkPage>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct BacklinkPage {
    pub page_id: i64,
    pub slug: String,
    pub title: String,
}

async fn can_view(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    page: &page::Model,
) -> Result<bool> {
    PermissionService::check_user_can(
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
    .await
}

/// Never return an incoming connection before validating the target and every source.
pub async fn get_page_backlinks(
    ctx: &ServiceContext<'_>,
    GetPageBacklinks { site_id, page_id }: GetPageBacklinks,
) -> Result<GetPageBacklinksOutput> {
    let target = PageService::get(ctx, site_id, Reference::Id(page_id)).await?;
    if !can_view(ctx, site_id, &target).await? {
        bail!(Error::new("page view denied", ErrorType::PermissionDenied));
    }

    let connections = PageConnection::find()
        .filter(page_connection::Column::ToPageId.eq(page_id))
        .filter(page_connection::Column::ConnectionType.is_in([
            ConnectionType::Link,
            ConnectionType::IncludeMessy,
            ConnectionType::IncludeElements,
        ]))
        .all(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new(
                "failed to fetch incoming page connections",
                ErrorType::PageLink,
            )
        })?;
    if connections.is_empty() {
        return Ok(GetPageBacklinksOutput {
            links: vec![],
            inclusions: vec![],
        });
    }

    let source_ids: HashSet<_> = connections.iter().map(|row| row.from_page_id).collect();
    let sources = page::Entity::find()
        .filter(page::Column::PageId.is_in(source_ids))
        .filter(page::Column::SiteId.eq(site_id))
        .filter(page::Column::DeletedAt.is_null())
        .all(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new("failed to fetch backlink source pages", ErrorType::PageLink)
        })?;
    let mut visible = Vec::new();
    for source in sources {
        if can_view(ctx, site_id, &source).await? {
            visible.push(source);
        }
    }
    let revision_ids: Vec<_> = visible
        .iter()
        .filter_map(|page| page.latest_revision_id)
        .collect();
    let revisions: HashMap<_, _> = page_revision::Entity::find()
        .filter(page_revision::Column::RevisionId.is_in(revision_ids))
        .filter(page_revision::Column::SiteId.eq(site_id))
        .all(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new(
                "failed to fetch current backlink titles",
                ErrorType::PageLink,
            )
        })?
        .into_iter()
        .map(|revision| (revision.revision_id, revision))
        .collect();

    let mut pages = HashMap::new();
    for source in visible {
        let Some(title) = source.latest_revision_id.and_then(|id| revisions.get(&id))
        else {
            continue;
        };
        if title.page_id != source.page_id {
            continue;
        }
        pages.insert(
            source.page_id,
            BacklinkPage {
                page_id: source.page_id,
                slug: source.slug,
                title: title.title.clone(),
            },
        );
    }
    let mut links = HashMap::new();
    let mut inclusions = HashMap::new();
    for connection in connections {
        let Some(source) = pages.get(&connection.from_page_id) else {
            continue;
        };
        match connection.connection_type {
            ConnectionType::Link => {
                links.insert(source.page_id, source.clone());
            }
            ConnectionType::IncludeMessy | ConnectionType::IncludeElements => {
                inclusions.insert(source.page_id, source.clone());
            }
            ConnectionType::Component | ConnectionType::Redirect => {
                unreachable!("filtered above")
            }
        }
    }
    let mut links: Vec<_> = links.into_values().collect();
    let mut inclusions: Vec<_> = inclusions.into_values().collect();
    links.sort_by(|a, b| a.slug.cmp(&b.slug).then(a.page_id.cmp(&b.page_id)));
    inclusions.sort_by(|a, b| a.slug.cmp(&b.slug).then(a.page_id.cmp(&b.page_id)));
    Ok(GetPageBacklinksOutput { links, inclusions })
}
