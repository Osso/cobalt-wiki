//! Bounded worker invocation. Caller schedules repeated calls after commits and at startup.
use super::{SearchDocument, SearchService, outbox, plain_body};
use crate::error::prelude::*;
use crate::models::{page, page_revision, search_index_pending, text};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryOrder, QuerySelect,
    Statement, TransactionTrait,
};

const BATCH_SIZE: u64 = 50;
const LOCK_KEY: i64 = 0x7365_6172_6368_7067;

/// Sync one bounded batch from committed DB state; returns number of rows examined.
/// The transaction-level lock serializes external writes across all worker processes.
/// It is intentionally held across Meilisearch task completion, not a row lock.
pub async fn process_one_batch(
    db: &DatabaseConnection,
    search: &SearchService,
) -> Result<usize> {
    let txn = db.begin().await.or_raise(|| {
        Error::new("failed to start search worker transaction", ErrorType::Page)
    })?;
    let lock = txn
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_try_advisory_xact_lock($1) AS acquired",
            [LOCK_KEY.into()],
        ))
        .await
        .or_raise(|| Error::new("failed to acquire search worker lock", ErrorType::Page))?
        .expect("advisory lock query returns one row");
    let acquired: bool = lock
        .try_get("", "acquired")
        .or_raise(|| Error::new("invalid search worker lock result", ErrorType::Page))?;
    if !acquired {
        return Ok(0);
    }

    let pending = search_index_pending::Entity::find()
        .order_by_asc(search_index_pending::Column::Generation)
        .limit(BATCH_SIZE)
        .all(&txn)
        .await
        .or_raise(|| {
            Error::new("failed to read pending search pages", ErrorType::Page)
        })?;
    let mut documents = Vec::new();
    let mut deleted = Vec::new();
    for row in &pending {
        match read_current_document(&txn, row.page_id).await? {
            Some(document) => documents.push(document),
            None => deleted.push(row.page_id),
        }
    }
    if !documents.is_empty() {
        search.upsert_batch(documents).await?;
    }
    for page_id in deleted {
        search.remove_page(page_id).await?;
    }
    for row in &pending {
        outbox::acknowledge(&txn, row.page_id, row.generation).await?;
    }
    txn.commit().await.or_raise(|| {
        Error::new(
            "failed to commit search worker acknowledgements",
            ErrorType::Page,
        )
    })?;
    Ok(pending.len())
}

async fn read_current_document(
    txn: &sea_orm::DatabaseTransaction,
    page_id: i64,
) -> Result<Option<SearchDocument>> {
    let page = page::Entity::find_by_id(page_id)
        .one(txn)
        .await
        .or_raise(|| Error::new("failed to read search page", ErrorType::Page))?;
    let Some(page) = page.filter(|page| page.deleted_at.is_none()) else {
        return Ok(None);
    };
    let revision_id = page.latest_revision_id.ok_or_else(|| {
        Error::new("search page missing current revision", ErrorType::Page)
    })?;
    let revision = page_revision::Entity::find_by_id(revision_id)
        .one(txn)
        .await
        .or_raise(|| Error::new("failed to read search revision", ErrorType::Page))?
        .ok_or_else(|| Error::new("search revision missing", ErrorType::Page))?;
    if revision.page_id != page_id || revision.site_id != page.site_id {
        return Err(Error::new(
            "search revision belongs to another page",
            ErrorType::Page,
        )
        .into());
    }
    let html = text::Entity::find_by_id(revision.compiled_body_html_hash)
        .one(txn)
        .await
        .or_raise(|| Error::new("failed to read search body", ErrorType::Page))?
        .ok_or_else(|| Error::new("search body missing", ErrorType::Page))?;
    Ok(Some(SearchDocument {
        page_id,
        site_id: page.site_id,
        revision_id,
        title: revision.title,
        slug: page.slug,
        tags: revision.tags,
        body: plain_body(&html.contents),
    }))
}
