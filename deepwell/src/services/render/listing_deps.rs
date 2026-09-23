//! Stored ListPages/CountPages selections per listing page, and the listing
//! pages a changed page affects.

use super::list_pages::{ListingFilter, ListingSubject};
use super::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement, Value};

/// Replace the selections a page body renders.
pub(super) async fn replace(
    ctx: &ServiceContext<'_>,
    page_id: i64,
    filters: &[ListingFilter],
) -> Result<()> {
    let txn = ctx.transaction();
    let make_error = || Error::new("failed to store page listings", ErrorType::Render);
    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "DELETE FROM page_listing WHERE page_id = $1",
        [Value::from(page_id)],
    ))
    .await
    .or_raise(make_error)?;
    for filter in filters {
        let selection = serde_json::to_value(filter).or_raise(make_error)?;
        txn.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO page_listing (page_id, selection) VALUES ($1, $2)",
            [Value::from(page_id), Value::from(selection)],
        ))
        .await
        .or_raise(make_error)?;
    }
    Ok(())
}

#[derive(Debug, FromQueryResult)]
struct StoredListing {
    page_id: i64,
    selection: serde_json::Value,
}

/// Listing pages of the site that could show any of `subjects` (a changed
/// page before and after the change).
pub async fn affected_pages(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    subjects: &[ListingSubject<'_>],
) -> Result<Vec<i64>> {
    let make_error =
        || Error::new("failed to read page listings", ErrorType::PageOutdater);
    let listings = StoredListing::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT l.page_id, l.selection FROM page_listing l
         JOIN page p ON p.page_id = l.page_id
         WHERE p.site_id = $1 AND p.deleted_at IS NULL",
        [Value::from(site_id)],
    ))
    .all(ctx.transaction())
    .await
    .or_raise(make_error)?;
    let mut pages = Vec::new();
    for StoredListing { page_id, selection } in listings {
        let filter: ListingFilter =
            serde_json::from_value(selection).or_raise(make_error)?;
        if !pages.contains(&page_id) && subjects.iter().any(|page| filter.matches(page)) {
            pages.push(page_id);
        }
    }
    Ok(pages)
}
