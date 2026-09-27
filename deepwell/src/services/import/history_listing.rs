//! Paginated read-only listing of native and preserved source history.

use super::history::authorize_read;
use super::history_listing_structs::*;
use super::history_structs::ImportedRevisionSummary;
use crate::error::prelude::*;
use crate::models::{imported_page_revision, page, page_revision, user, wikidot_user};
use crate::services::ServiceContext;
use crate::types::PageRevisionType;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Select,
};
use std::collections::HashMap;

#[derive(Debug)]
pub struct HistoryListingService;

fn invalid(message: &str) -> ExnError {
    Error::new(message, ErrorType::DatabaseImport).into()
}

impl HistoryListingService {
    pub async fn list(
        ctx: &ServiceContext<'_>,
        input: ReadPageHistory,
    ) -> Result<HistoryListing> {
        let offset = validate_pagination(&input)?;
        authorize_read(ctx, input.site_id, input.page_id).await?;
        let latest_revision_id = load_current_revision_id(ctx, &input).await?;
        let available = query_history_availability(ctx, &input).await?;
        let (total, rows) = match input.origin {
            HistoryOrigin::Wikidot => query_imported_history(ctx, &input, offset).await?,
            HistoryOrigin::Local => {
                query_native_history(ctx, &input, offset, latest_revision_id).await?
            }
        };
        Ok(HistoryListing {
            origin: input.origin,
            page: input.page,
            per_page: input.per_page,
            total,
            total_pages: total.div_ceil(input.per_page as u64),
            available,
            rows,
        })
    }
}

fn validate_pagination(input: &ReadPageHistory) -> Result<i64> {
    if input.page < 1 {
        return Err(invalid("history page must be positive"));
    }
    if ![10, 20, 50, 100, 200].contains(&input.per_page) {
        return Err(invalid(
            "history per_page must be one of 10, 20, 50, 100, 200",
        ));
    }
    (input.page - 1)
        .checked_mul(input.per_page)
        .ok_or_else(|| invalid("history page offset exceeds supported range"))
}

async fn load_current_revision_id(
    ctx: &ServiceContext<'_>,
    input: &ReadPageHistory,
) -> Result<Option<i64>> {
    let current = page::Entity::find()
        .filter(page::Column::SiteId.eq(input.site_id))
        .filter(page::Column::PageId.eq(input.page_id))
        .one(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new("failed to read current page", ErrorType::DatabaseImport)
        })?
        .ok_or_else(|| invalid("history target page does not exist"))?;
    Ok(current.latest_revision_id)
}

async fn query_history_availability(
    ctx: &ServiceContext<'_>,
    input: &ReadPageHistory,
) -> Result<HistoryAvailability> {
    let imported = imported_page_revision::Entity::find()
        .filter(imported_page_revision::Column::SiteId.eq(input.site_id))
        .filter(imported_page_revision::Column::PageId.eq(input.page_id));
    let native = page_revision::Entity::find()
        .filter(page_revision::Column::SiteId.eq(input.site_id))
        .filter(page_revision::Column::PageId.eq(input.page_id));
    let wikidot = imported.count(ctx.transaction()).await.or_raise(|| {
        Error::new(
            "failed to count imported history",
            ErrorType::DatabaseImport,
        )
    })? > 0;
    let local = native.count(ctx.transaction()).await.or_raise(|| {
        Error::new("failed to count native history", ErrorType::DatabaseImport)
    })? > 0;
    Ok(HistoryAvailability { wikidot, local })
}

fn is_unfiltered(filters: &HistoryFilters) -> bool {
    filters.all
        || ![
            filters.source,
            filters.title,
            filters.r#move,
            filters.tags,
            filters.meta,
            filters.files,
        ]
        .contains(&true)
}

async fn query_imported_history(
    ctx: &ServiceContext<'_>,
    input: &ReadPageHistory,
    offset: i64,
) -> Result<(u64, Vec<HistoryListingRow>)> {
    let mut query = imported_page_revision::Entity::find()
        .filter(imported_page_revision::Column::SiteId.eq(input.site_id))
        .filter(imported_page_revision::Column::PageId.eq(input.page_id));
    if !is_unfiltered(&input.filters) {
        query = query.filter(imported_condition(&input.filters));
    }
    let total = query.clone().count(ctx.transaction()).await.or_raise(|| {
        Error::new(
            "failed to count filtered imported history",
            ErrorType::DatabaseImport,
        )
    })?;
    let models = query_imported_summaries(ctx, query, input.per_page, offset).await?;
    Ok((total, imported_rows(ctx, models).await?))
}

async fn query_imported_summaries(
    ctx: &ServiceContext<'_>,
    query: Select<imported_page_revision::Entity>,
    per_page: i64,
    offset: i64,
) -> Result<Vec<ImportedRevisionSummary>> {
    use imported_page_revision::Column as HistoryColumn;
    query
        .select_only()
        .columns([
            HistoryColumn::SourcePageId,
            HistoryColumn::SourceRevisionId,
            HistoryColumn::SourceRevisionNumber,
            HistoryColumn::SourceAuthorId,
            HistoryColumn::SourceCreatedAt,
            HistoryColumn::SourceComments,
            HistoryColumn::SourceFlags,
            HistoryColumn::SourceTitle,
            HistoryColumn::SourceSlug,
            HistoryColumn::SourceTags,
            HistoryColumn::Representation,
        ])
        .order_by_desc(HistoryColumn::SourceRevisionNumber)
        .limit(per_page as u64)
        .offset(offset as u64)
        .into_model::<ImportedRevisionSummary>()
        .all(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new("failed to list imported history", ErrorType::DatabaseImport)
        })
}

async fn query_native_history(
    ctx: &ServiceContext<'_>,
    input: &ReadPageHistory,
    offset: i64,
    latest_revision_id: Option<i64>,
) -> Result<(u64, Vec<HistoryListingRow>)> {
    let mut query = page_revision::Entity::find()
        .filter(page_revision::Column::SiteId.eq(input.site_id))
        .filter(page_revision::Column::PageId.eq(input.page_id));
    if !is_unfiltered(&input.filters) {
        query = query.filter(native_condition(&input.filters));
    }
    let total = query.clone().count(ctx.transaction()).await.or_raise(|| {
        Error::new(
            "failed to count filtered native history",
            ErrorType::DatabaseImport,
        )
    })?;
    let models = query
        .order_by_desc(page_revision::Column::RevisionNumber)
        .limit(input.per_page as u64)
        .offset(offset as u64)
        .all(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new("failed to list native history", ErrorType::DatabaseImport)
        })?;
    Ok((total, native_rows(ctx, models, latest_revision_id).await?))
}

fn imported_condition(filters: &HistoryFilters) -> Condition {
    let mut condition = Condition::any();
    for (enabled, predicate) in [
        (filters.source, "'S' = ANY(source_flags)"),
        (filters.title, "'T' = ANY(source_flags)"),
        (filters.r#move, "'R' = ANY(source_flags)"),
        (filters.tags, "'A' = ANY(source_flags)"),
        (filters.meta, "'M' = ANY(source_flags)"),
        (filters.files, "'F' = ANY(source_flags)"),
    ] {
        if enabled {
            condition = condition.add(Expr::cust(predicate));
        }
    }
    condition
}

fn native_condition(filters: &HistoryFilters) -> Condition {
    let mut condition = Condition::any();
    if filters.source {
        condition = condition.add(Expr::cust("'wikitext' = ANY(changes)"));
    }
    if filters.title {
        condition = condition.add(Expr::cust("'title' = ANY(changes)"));
    }
    if filters.r#move {
        condition =
            condition.add(page_revision::Column::RevisionType.eq(PageRevisionType::Move));
    }
    if filters.tags {
        condition = condition.add(Expr::cust("'tags' = ANY(changes)"));
    }
    if filters.meta {
        condition = condition.add(Expr::cust("'alt_title' = ANY(changes)"));
    }
    // Native page revisions have no file-change signal; files-only has no native matches.
    if filters.files
        && ![
            filters.source,
            filters.title,
            filters.r#move,
            filters.tags,
            filters.meta,
        ]
        .contains(&true)
    {
        condition = condition.add(Expr::cust("FALSE"));
    }
    condition
}

async fn imported_rows(
    ctx: &ServiceContext<'_>,
    models: Vec<ImportedRevisionSummary>,
) -> Result<Vec<HistoryListingRow>> {
    let ids: Vec<i32> = models
        .iter()
        .filter_map(|row| row.source_author_id.and_then(|id| i32::try_from(id).ok()))
        .collect();
    let names: HashMap<i64, String> = wikidot_user::Entity::find()
        .filter(wikidot_user::Column::UserId.is_in(ids))
        .all(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new(
                "failed to read stored Wikidot author names",
                ErrorType::DatabaseImport,
            )
        })?
        .into_iter()
        .filter_map(|row| row.name.map(|name| (i64::from(row.user_id), name)))
        .collect();
    // Imported members keep their Wikidot user ID as their local user ID.
    let slugs = local_author_slugs(ctx, &models, |row| row.source_author_id).await?;
    Ok(models
        .into_iter()
        .map(|row| HistoryListingRow {
            id: row.source_revision_id,
            number: row.source_revision_number,
            flags: row.source_flags,
            author_id: row.source_author_id,
            author_name: row.source_author_id.and_then(|id| names.get(&id).cloned()),
            author_slug: row.source_author_id.and_then(|id| slugs.get(&id).cloned()),
            created_at: row.source_created_at,
            comments: row.source_comments,
            is_current: false,
            representation: Some(row.representation),
        })
        .collect())
}

async fn local_authors(
    ctx: &ServiceContext<'_>,
    ids: Vec<i64>,
) -> Result<HashMap<i64, (String, String)>> {
    Ok(user::Entity::find()
        .filter(user::Column::UserId.is_in(ids))
        .all(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new(
                "failed to read local author names",
                ErrorType::DatabaseImport,
            )
        })?
        .into_iter()
        .map(|row| (row.user_id, (row.name, row.slug)))
        .collect())
}

async fn local_author_slugs<T>(
    ctx: &ServiceContext<'_>,
    rows: &[T],
    author_id: impl Fn(&T) -> Option<i64>,
) -> Result<HashMap<i64, String>> {
    let ids = rows.iter().filter_map(author_id).collect();
    Ok(local_authors(ctx, ids)
        .await?
        .into_iter()
        .map(|(id, (_, slug))| (id, slug))
        .collect())
}

async fn native_rows(
    ctx: &ServiceContext<'_>,
    models: Vec<page_revision::Model>,
    latest_revision_id: Option<i64>,
) -> Result<Vec<HistoryListingRow>> {
    let ids: Vec<i64> = models.iter().map(|row| row.user_id).collect();
    let authors = local_authors(ctx, ids).await?;
    Ok(models
        .into_iter()
        .map(|row| {
            let flags = native_flags(&row);
            HistoryListingRow {
                id: row.revision_id,
                number: row.revision_number,
                flags,
                author_id: Some(row.user_id),
                author_name: authors.get(&row.user_id).map(|(name, _)| name.clone()),
                author_slug: authors.get(&row.user_id).map(|(_, slug)| slug.clone()),
                created_at: row.created_at,
                comments: if row.hidden.iter().any(|field| field == "comments") {
                    String::new()
                } else {
                    row.comments
                },
                is_current: Some(row.revision_id) == latest_revision_id,
                representation: None,
            }
        })
        .collect())
}

fn native_flags(row: &page_revision::Model) -> Vec<String> {
    let mut flags = Vec::new();
    if row.revision_type == PageRevisionType::Create {
        flags.push("N".into());
    }
    if row.changes.iter().any(|change| change == "wikitext") {
        flags.push("S".into());
    }
    if row.changes.iter().any(|change| change == "title") {
        flags.push("T".into());
    }
    if row.revision_type == PageRevisionType::Move {
        flags.push("R".into());
    }
    if row.changes.iter().any(|change| change == "tags") {
        flags.push("A".into());
    }
    if row.changes.iter().any(|change| change == "alt_title") {
        flags.push("M".into());
    }
    flags
}
