//! Source-compatible slug-prefix suggestions for the current editor site.

use super::prelude::*;
use crate::models::{page, page_revision};
use crate::services::permission::{CheckPermissionContext, PermissionService};
use crate::types::{Action, Permission, Reference, Resource};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use sea_query::{Expr, extension::postgres::PgExpr};
use serde::{Deserialize, Serialize};

const SUGGESTION_LIMIT: usize = 20;
const CANDIDATE_BATCH: u64 = 40;

#[derive(Debug, Deserialize)]
struct EditorPagesRequest {
    query: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditorPage {
    pub slug: String,
    pub title: String,
}

pub async fn editor_pages(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<Vec<EditorPage>> {
    let input: EditorPagesRequest = parse!(params, Request);
    if input.query.trim().len() < 2 || input.query.len() > 200 {
        return Err(
            Error::new("invalid editor page query", ErrorType::BadRequest).into(),
        );
    }
    let site_id = ctx.request().site_id()?;
    let user_id = ctx.request().user_id;
    let query = input.query.replace(' ', "-");
    let pattern = format!("{}%", escape_like(&query));
    let mut cursor: Option<(String, i64)> = None;
    let mut suggestions = Vec::new();

    loop {
        let candidates =
            query_candidates(ctx, site_id, &pattern, cursor.as_ref()).await?;
        let count = candidates.len();
        for candidate in candidates {
            cursor = Some((candidate.slug.clone(), candidate.page_id));
            if let Some(suggestion) =
                load_visible_suggestion(ctx, site_id, user_id, candidate).await?
            {
                suggestions.push(suggestion);
                if suggestions.len() == SUGGESTION_LIMIT {
                    return Ok(suggestions);
                }
            }
        }
        if count < CANDIDATE_BATCH as usize {
            return Ok(suggestions);
        }
    }
}

async fn query_candidates(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    pattern: &str,
    cursor: Option<&(String, i64)>,
) -> Result<Vec<page::Model>> {
    let mut filter = Condition::all()
        .add(page::Column::SiteId.eq(site_id))
        .add(page::Column::DeletedAt.is_null())
        .add(page::Column::LatestRevisionId.is_not_null())
        .add(Expr::col(page::Column::Slug).ilike(pattern));
    if let Some((slug, id)) = cursor {
        filter = filter.add(
            Condition::any()
                .add(page::Column::Slug.gt(slug.as_str()))
                .add(
                    Condition::all()
                        .add(page::Column::Slug.eq(slug.as_str()))
                        .add(page::Column::PageId.gt(*id)),
                ),
        );
    }
    page::Entity::find()
        .filter(filter)
        .order_by_asc(page::Column::Slug)
        .order_by_asc(page::Column::PageId)
        .limit(CANDIDATE_BATCH)
        .all(ctx.transaction())
        .await
        .or_raise(|| Error::new("failed to list editor page candidates", ErrorType::Page))
}

fn escape_like(query: &str) -> String {
    let mut escaped = String::with_capacity(query.len());
    for character in query.chars() {
        if matches!(character, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

async fn load_visible_suggestion(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    user_id: Option<i64>,
    candidate: page::Model,
) -> Result<Option<EditorPage>> {
    let allowed = PermissionService::check_user_can(
        ctx,
        &CheckPermissionContext {
            user_id,
            site_id,
            page_reference: Some(Reference::Id(candidate.page_id)),
        },
        Permission {
            resource_type: Resource::Page,
            resource_category: Some(Reference::Id(candidate.page_category_id)),
            action: Action::View,
        },
    )
    .await?;
    if !allowed {
        return Ok(None);
    }
    let Some(revision_id) = candidate.latest_revision_id else {
        return Ok(None);
    };
    let revision = page_revision::Entity::find_by_id(revision_id)
        .one(ctx.transaction())
        .await
        .or_raise(|| Error::new("failed to read editor page title", ErrorType::Page))?;
    Ok(revision
        .filter(|revision| {
            revision.site_id == site_id && revision.page_id == candidate.page_id
        })
        .map(|revision| EditorPage {
            slug: candidate.slug,
            title: revision.title,
        }))
}
