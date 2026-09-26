//! Paginated read-only listing of native and preserved source history.

use super::history::authorize_read;
use super::history_listing_structs::*;
use crate::error::prelude::*;
use crate::models::{imported_page_revision, page, page_revision, user, wikidot_user};
use crate::services::ServiceContext;
use crate::types::PageRevisionType;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
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
        if input.page < 1 {
            return Err(invalid("history page must be positive"));
        }
        if ![10, 20, 50, 100, 200].contains(&input.per_page) {
            return Err(invalid(
                "history per_page must be one of 10, 20, 50, 100, 200",
            ));
        }
        let offset = (input.page - 1)
            .checked_mul(input.per_page)
            .ok_or_else(|| invalid("history page offset exceeds supported range"))?;
        authorize_read(ctx, input.site_id, input.page_id).await?;
        let current = page::Entity::find()
            .filter(page::Column::SiteId.eq(input.site_id))
            .filter(page::Column::PageId.eq(input.page_id))
            .one(ctx.transaction())
            .await
            .or_raise(|| {
                Error::new("failed to read current page", ErrorType::DatabaseImport)
            })?
            .ok_or_else(|| invalid("history target page does not exist"))?;

        let imported = imported_page_revision::Entity::find()
            .filter(imported_page_revision::Column::SiteId.eq(input.site_id))
            .filter(imported_page_revision::Column::PageId.eq(input.page_id));
        let native = page_revision::Entity::find()
            .filter(page_revision::Column::SiteId.eq(input.site_id))
            .filter(page_revision::Column::PageId.eq(input.page_id));
        let (imported_available, native_available) = (
            imported
                .clone()
                .count(ctx.transaction())
                .await
                .or_raise(|| {
                    Error::new(
                        "failed to count imported history",
                        ErrorType::DatabaseImport,
                    )
                })?
                > 0,
            native.clone().count(ctx.transaction()).await.or_raise(|| {
                Error::new("failed to count native history", ErrorType::DatabaseImport)
            })? > 0,
        );
        let filters = &input.filters;
        let filtered = filters.all
            || ![
                filters.source,
                filters.title,
                filters.r#move,
                filters.meta,
                filters.files,
            ]
            .contains(&true);
        let (total, rows) = match input.origin {
            HistoryOrigin::Wikidot => {
                let mut query = imported;
                if !filtered {
                    query = query.filter(imported_condition(filters));
                }
                let total =
                    query.clone().count(ctx.transaction()).await.or_raise(|| {
                        Error::new(
                            "failed to count filtered imported history",
                            ErrorType::DatabaseImport,
                        )
                    })?;
                let models = query
                    .order_by_desc(imported_page_revision::Column::SourceRevisionNumber)
                    .limit(input.per_page as u64)
                    .offset(offset as u64)
                    .all(ctx.transaction())
                    .await
                    .or_raise(|| {
                        Error::new(
                            "failed to list imported history",
                            ErrorType::DatabaseImport,
                        )
                    })?;
                (total, imported_rows(ctx, models).await?)
            }
            HistoryOrigin::Local => {
                let mut query = native;
                if !filtered {
                    query = query.filter(native_condition(filters));
                }
                let total =
                    query.clone().count(ctx.transaction()).await.or_raise(|| {
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
                        Error::new(
                            "failed to list native history",
                            ErrorType::DatabaseImport,
                        )
                    })?;
                (
                    total,
                    native_rows(ctx, models, current.latest_revision_id).await?,
                )
            }
        };
        Ok(HistoryListing {
            origin: input.origin,
            page: input.page,
            per_page: input.per_page,
            total,
            total_pages: total.div_ceil(input.per_page as u64),
            available: HistoryAvailability {
                wikidot: imported_available,
                local: native_available,
            },
            rows,
        })
    }
}

fn imported_condition(filters: &HistoryFilters) -> Condition {
    let mut condition = Condition::any();
    for (enabled, predicate) in [
        (filters.source, "'S' = ANY(source_flags)"),
        (filters.title, "'T' = ANY(source_flags)"),
        (filters.r#move, "'R' = ANY(source_flags)"),
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
    if filters.meta {
        condition = condition.add(Expr::cust(
            "('tags' = ANY(changes) OR 'alt_title' = ANY(changes))",
        ));
    }
    // Native page revisions have no file-change signal; files-only has no native matches.
    if filters.files
        && ![filters.source, filters.title, filters.r#move, filters.meta].contains(&true)
    {
        condition = condition.add(Expr::cust("FALSE"));
    }
    condition
}

async fn imported_rows(
    ctx: &ServiceContext<'_>,
    models: Vec<imported_page_revision::Model>,
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
    Ok(models
        .into_iter()
        .map(|row| HistoryListingRow {
            id: row.source_revision_id,
            number: row.source_revision_number,
            flags: row.source_flags,
            author_id: row.source_author_id,
            author_name: row.source_author_id.and_then(|id| names.get(&id).cloned()),
            created_at: row.source_created_at,
            comments: row.source_comments,
            is_current: false,
            representation: Some(row.representation),
        })
        .collect())
}

async fn native_rows(
    ctx: &ServiceContext<'_>,
    models: Vec<page_revision::Model>,
    latest_revision_id: Option<i64>,
) -> Result<Vec<HistoryListingRow>> {
    let ids: Vec<i64> = models.iter().map(|row| row.user_id).collect();
    let names: HashMap<i64, String> = user::Entity::find()
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
        .map(|row| (row.user_id, row.name))
        .collect();
    Ok(models
        .into_iter()
        .map(|row| {
            let flags = native_flags(&row);
            HistoryListingRow {
                id: row.revision_id,
                number: row.revision_number,
                flags,
                author_id: Some(row.user_id),
                author_name: names.get(&row.user_id).cloned(),
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
    if row
        .changes
        .iter()
        .any(|change| change == "tags" || change == "alt_title")
    {
        flags.push("M".into());
    }
    flags
}
