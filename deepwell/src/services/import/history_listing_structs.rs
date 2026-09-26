use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryOrigin {
    Wikidot,
    Local,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryFilters {
    pub all: bool,
    pub source: bool,
    pub title: bool,
    pub r#move: bool,
    pub meta: bool,
    pub files: bool,
}

#[derive(Debug, Deserialize)]
pub struct ReadPageHistory {
    pub site_id: i64,
    pub page_id: i64,
    pub origin: HistoryOrigin,
    #[serde(default = "first_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    #[serde(default)]
    pub filters: HistoryFilters,
}

fn first_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    20
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryAvailability {
    pub wikidot: bool,
    pub local: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryListing {
    pub origin: HistoryOrigin,
    pub page: i64,
    pub per_page: i64,
    pub total: u64,
    pub total_pages: u64,
    pub available: HistoryAvailability,
    pub rows: Vec<HistoryListingRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryListingRow {
    pub id: i64,
    pub number: i32,
    pub flags: Vec<String>,
    pub author_id: Option<i64>,
    pub author_name: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub comments: String,
    pub is_current: bool,
    pub representation: Option<String>,
}
