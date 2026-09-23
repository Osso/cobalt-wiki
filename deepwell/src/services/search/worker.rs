//! Search outbox worker.
use super::{SearchDocument, SearchService, outbox, plain_body};
use crate::error::prelude::*;
use crate::models::{page, page_revision, search_index_pending, text};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, EntityTrait,
    QueryOrder, QuerySelect, Statement, TransactionTrait,
};
use std::future::Future;
use tokio::time::{Duration, sleep};

const BATCH_SIZE: u64 = 50;
const LOCK_KEY: i64 = 0x7365_6172_6368_7067;
const IDLE_DELAY: Duration = Duration::from_secs(1);

/// Ensure the index once, then keep draining committed outbox work until cancelled or failed.
pub async fn run(db: &DatabaseConnection, search: &SearchService) -> Result<()> {
    search.ensure_index().await?;
    run_batches(|| process_one_batch(db, search)).await
}

async fn run_batches<F, Fut>(mut process_batch: F) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<usize>>,
{
    loop {
        if process_batch().await? == 0 {
            sleep(IDLE_DELAY).await;
        }
    }
}

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
    if !acquire_worker_lock(&txn).await? {
        return Ok(0);
    }

    let pending = read_pending_pages(&txn).await?;
    let (documents, deleted) = read_batch_documents(&txn, &pending).await?;
    publish_batch(search, documents, deleted).await?;
    acknowledge_batch(&txn, &pending).await?;
    txn.commit().await.or_raise(|| {
        Error::new(
            "failed to commit search worker acknowledgements",
            ErrorType::Page,
        )
    })?;
    Ok(pending.len())
}

async fn acquire_worker_lock(txn: &DatabaseTransaction) -> Result<bool> {
    let lock = txn
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_try_advisory_xact_lock($1) AS acquired",
            [LOCK_KEY.into()],
        ))
        .await
        .or_raise(|| Error::new("failed to acquire search worker lock", ErrorType::Page))?
        .expect("advisory lock query returns one row");
    lock.try_get("", "acquired")
        .or_raise(|| Error::new("invalid search worker lock result", ErrorType::Page))
}

async fn read_pending_pages(
    txn: &DatabaseTransaction,
) -> Result<Vec<search_index_pending::Model>> {
    search_index_pending::Entity::find()
        .order_by_asc(search_index_pending::Column::Generation)
        .limit(BATCH_SIZE)
        .all(txn)
        .await
        .or_raise(|| Error::new("failed to read pending search pages", ErrorType::Page))
}

async fn read_batch_documents(
    txn: &DatabaseTransaction,
    pending: &[search_index_pending::Model],
) -> Result<(Vec<SearchDocument>, Vec<i64>)> {
    let mut documents = Vec::new();
    let mut deleted = Vec::new();
    for row in pending {
        match read_current_document(txn, row.page_id).await? {
            Some(document) => documents.push(document),
            None => deleted.push(row.page_id),
        }
    }
    Ok((documents, deleted))
}

async fn publish_batch(
    search: &SearchService,
    documents: Vec<SearchDocument>,
    deleted: Vec<i64>,
) -> Result<()> {
    if !documents.is_empty() {
        search.upsert_batch(documents).await?;
    }
    for page_id in deleted {
        search.remove_page(page_id).await?;
    }
    Ok(())
}

async fn acknowledge_batch(
    txn: &DatabaseTransaction,
    pending: &[search_index_pending::Model],
) -> Result<()> {
    for row in pending {
        outbox::acknowledge(txn, row.page_id, row.generation).await?;
    }
    Ok(())
}

#[cfg(test)]
mod scheduling_tests {
    use super::*;
    use tokio::sync::mpsc;
    use tokio::time::{Duration, Instant, timeout};

    #[tokio::test]
    async fn drains_nonempty_batches_before_waiting_on_empty_batch() {
        let (calls_tx, mut calls_rx) = mpsc::unbounded_channel();
        let worker = tokio::spawn(async move {
            let mut results = [50, 1, 0, 0].into_iter();
            run_batches(|| {
                calls_tx.send(Instant::now()).unwrap();
                let count = results.next().unwrap_or(0);
                async move { Ok(count) }
            })
            .await
        });

        let first = timeout(Duration::from_secs(2), calls_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let second = timeout(Duration::from_millis(500), calls_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let third = timeout(Duration::from_millis(500), calls_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(second.duration_since(first) < Duration::from_millis(500));
        assert!(third.duration_since(second) < Duration::from_millis(500));
        assert!(
            timeout(Duration::from_millis(700), calls_rx.recv())
                .await
                .is_err()
        );
        assert!(
            timeout(Duration::from_millis(700), calls_rx.recv())
                .await
                .unwrap()
                .is_some()
        );
        worker.abort();
        worker.await.unwrap_err();
    }

    #[tokio::test]
    async fn exits_on_batch_error_instead_of_retrying() {
        let mut calls = 0;
        let result = timeout(
            Duration::from_secs(2),
            run_batches(|| {
                calls += 1;
                async {
                    Err(Error::new("permanent batch failure", ErrorType::Page).into())
                }
            }),
        )
        .await
        .unwrap();
        assert!(result.is_err());
        assert_eq!(calls, 1);
    }
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
