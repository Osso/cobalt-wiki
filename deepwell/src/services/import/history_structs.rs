use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::models::imported_page_revision::Model;

#[derive(Debug, Deserialize)]
pub struct ImportHistory {
    pub site_id: i64,
    pub page_id: i64,
    pub source_page_id: i64,
    pub expected_revision_id: i64,
    pub revisions: Vec<ImportHistoryRevision>,
}

#[derive(Debug, Deserialize)]
pub struct ImportHistoryRevision {
    pub source_revision_id: i64,
    pub source_revision_number: i32,
    pub source_author_id: Option<i64>,
    #[serde(with = "time::serde::rfc3339")]
    pub source_created_at: OffsetDateTime,
    pub source_comments: String,
    pub source_flags: Vec<String>,
    pub source_title: Option<String>,
    pub source_slug: Option<String>,
    pub source_tags: Option<Vec<String>>,
    pub wikitext: String,
    pub raw_source_html: String,
    #[serde(with = "time::serde::rfc3339")]
    pub acquired_at: OffsetDateTime,
    pub representation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportHistoryOutput {
    pub inserted: usize,
}

#[derive(Debug, Deserialize)]
pub struct ReadImportedHistory {
    pub site_id: i64,
    pub page_id: i64,
    pub before_revision: Option<i32>,
    pub limit: u64,
    pub user_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ReadImportedRevision {
    pub site_id: i64,
    pub page_id: i64,
    pub source_revision_number: i32,
    pub user_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportedRevisionSummary {
    pub source_page_id: i64,
    pub source_revision_id: i64,
    pub source_revision_number: i32,
    pub source_author_id: Option<i64>,
    #[serde(with = "time::serde::rfc3339")]
    pub source_created_at: OffsetDateTime,
    pub source_comments: String,
    pub source_flags: Vec<String>,
    pub source_title: Option<String>,
    pub source_slug: Option<String>,
    pub source_tags: Option<Vec<String>>,
    pub representation: String,
}

impl From<Model> for ImportedRevisionSummary {
    fn from(row: Model) -> Self {
        Self {
            source_page_id: row.source_page_id,
            source_revision_id: row.source_revision_id,
            source_revision_number: row.source_revision_number,
            source_author_id: row.source_author_id,
            source_created_at: row.source_created_at,
            source_comments: row.source_comments,
            source_flags: row.source_flags,
            source_title: row.source_title,
            source_slug: row.source_slug,
            source_tags: row.source_tags,
            representation: row.representation,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportedRevisionSource {
    pub metadata: ImportedRevisionSummary,
    pub wikitext: String,
}
