//! Transactional search outbox. Enqueue on the same transaction as the page edit.
use crate::error::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement};

/// Enqueue the current page state, including deletion. The caller owns commit/rollback.
pub async fn enqueue(txn: &DatabaseTransaction, page_id: i64) -> Result<()> {
    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO search_index_pending (page_id) VALUES ($1) \
         ON CONFLICT (page_id) DO UPDATE SET generation = EXCLUDED.generation",
        [page_id.into()],
    ))
    .await
    .or_raise(|| Error::new("failed to enqueue search page", ErrorType::Page))?;
    Ok(())
}

/// Acknowledge only the generation sent to Meilisearch; concurrent edits remain queued.
pub async fn acknowledge(
    txn: &DatabaseTransaction,
    page_id: i64,
    generation: i64,
) -> Result<bool> {
    let result = txn
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM search_index_pending WHERE page_id = $1 AND generation = $2",
            [page_id.into(), generation.into()],
        ))
        .await
        .or_raise(|| Error::new("failed to acknowledge search page", ErrorType::Page))?;
    Ok(result.rows_affected() == 1)
}
