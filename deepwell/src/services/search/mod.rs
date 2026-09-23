//! Server-side Meilisearch index and permission-gated current-page search.

pub mod outbox;
pub mod worker;

use crate::error::prelude::*;
use crate::models::{page, page_revision};
use crate::services::permission::{CheckPermissionContext, PermissionService};
use crate::services::{PageService, ServiceContext, TextService};
use crate::types::{Action, Permission, Reference, Resource};
use reqwest::{Client, Method, StatusCode};
use scraper::{Html, Node};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::future::Future;
use std::time::Duration;

const INDEX: &str = "pages";
const BATCH: usize = 50;
const MAX_CANDIDATES: usize = 1000;

#[derive(Debug, Clone, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchDocument {
    pub page_id: i64,
    pub site_id: i64,
    pub revision_id: i64,
    pub title: String,
    pub slug: String,
    pub tags: Vec<String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchHit {
    pub page_id: i64,
    pub title: String,
    pub slug: String,
    pub tags: Vec<String>,
    pub snippet: String,
}

impl SearchHit {
    fn from_current_document(doc: &SearchDocument, current_html: &str) -> Option<Self> {
        (doc.body == plain_body(current_html)).then(|| Self::from_document(doc))
    }

    fn from_document(doc: &SearchDocument) -> Self {
        Self {
            page_id: doc.page_id,
            title: doc.title.clone(),
            slug: doc.slug.clone(),
            tags: doc.tags.clone(),
            snippet: doc.body.chars().take(200).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchPage {
    pub hits: Vec<SearchHit>,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
struct MeiliHits {
    hits: Vec<SearchDocument>,
}

#[derive(Debug, Deserialize)]
struct TaskCreated {
    #[serde(rename = "taskUid")]
    task_uid: u64,
}

#[derive(Debug, Deserialize)]
struct TaskStatus {
    status: String,
}

/// Only instantiate on the server: the master key must never reach an API response or browser.
#[derive(Clone)]
pub struct SearchService {
    url: String,
    key: String,
    client: Client,
}

impl std::fmt::Debug for SearchService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SearchService")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl SearchService {
    pub fn from_env() -> Result<Self> {
        let url = std::env::var("MEILISEARCH_URL")
            .map_err(|_| Error::new("MEILISEARCH_URL is required", ErrorType::Request))?;
        let key = std::env::var("MEILISEARCH_MASTER_KEY").map_err(|_| {
            Error::new("MEILISEARCH_MASTER_KEY is required", ErrorType::Request)
        })?;
        if key.is_empty() || !(url.starts_with("http://") || url.starts_with("https://"))
        {
            return Err(Error::new(
                "invalid Meilisearch server configuration",
                ErrorType::Request,
            )
            .into());
        }
        Ok(Self::new(url, key))
    }

    pub fn new(url: String, key: String) -> Self {
        Self {
            url: url.trim_end_matches('/').into(),
            key,
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
        }
    }

    /// Configure the site filter before searching/indexing. Caller runs once at initialization.
    pub async fn ensure_index(&self) -> Result<()> {
        let response = self
            .client
            .get(format!("{}/indexes/{INDEX}", self.url))
            .bearer_auth(&self.key)
            .send()
            .await
            .or_raise(|| {
                Error::new("failed to inspect Meilisearch index", ErrorType::Request)
            })?;
        match response.status() {
            StatusCode::NOT_FOUND => {
                let task: TaskCreated = self
                    .request(
                        Method::POST,
                        "/indexes",
                        Some(serde_json::json!({"uid": INDEX, "primaryKey": "page_id"})),
                    )
                    .await?;
                self.wait_task(task.task_uid).await?;
            }
            status if status.is_success() => (),
            status => {
                return Err(Error::new(
                    format!("Meilisearch index inspection failed: HTTP {status}"),
                    ErrorType::Request,
                )
                .into());
            }
        }
        let task: TaskCreated = self
            .request(
                Method::PUT,
                &format!("/indexes/{INDEX}/settings/filterable-attributes"),
                Some(serde_json::json!(["site_id"])),
            )
            .await?;
        self.wait_task(task.task_uid).await
    }

    /// Called after DB commit: never index uncommitted revision state.
    pub async fn index_page(&self, ctx: &ServiceContext<'_>, page_id: i64) -> Result<()> {
        let Some(page) = PageService::get_direct_optional(ctx, page_id, false).await?
        else {
            return self.remove_page(page_id).await;
        };
        let Some(revision_id) = page.latest_revision_id else {
            return Err(
                Error::new("page missing current revision", ErrorType::Page).into()
            );
        };
        let revision = page_revision::Entity::find_by_id(revision_id)
            .one(ctx.transaction())
            .await
            .or_raise(|| {
                Error::new(
                    "failed to read current revision for search",
                    ErrorType::Page,
                )
            })?
            .ok_or_else(|| {
                Error::new("current revision missing for search", ErrorType::Page)
            })?;
        if revision.page_id != page_id || revision.site_id != page.site_id {
            return Err(Error::new(
                "current revision does not belong to page",
                ErrorType::Page,
            )
            .into());
        }
        let html = TextService::get(ctx, &revision.compiled_body_html_hash).await?;
        self.upsert(SearchDocument {
            page_id,
            site_id: page.site_id,
            revision_id,
            title: revision.title,
            slug: page.slug,
            tags: revision.tags,
            body: plain_body(&html),
        })
        .await
    }

    pub async fn upsert(&self, document: SearchDocument) -> Result<()> {
        self.upsert_batch(vec![document]).await
    }

    pub async fn upsert_batch(&self, documents: Vec<SearchDocument>) -> Result<()> {
        let task: TaskCreated = self
            .request(
                Method::POST,
                &format!("/indexes/{INDEX}/documents?primaryKey=page_id"),
                Some(documents),
            )
            .await?;
        self.wait_task(task.task_uid).await
    }

    pub async fn remove_page(&self, page_id: i64) -> Result<()> {
        let task: TaskCreated = self
            .request::<_, TaskCreated>(
                Method::DELETE,
                &format!("/indexes/{INDEX}/documents/{page_id}"),
                None::<()>,
            )
            .await?;
        self.wait_task(task.task_uid).await
    }

    /// Raw hits never escape this method. Every candidate is checked against current DB state
    /// and request actor before constructing any output, and offset counts visible pages only.
    pub async fn page_search(
        &self,
        ctx: &ServiceContext<'_>,
        request: SearchRequest,
    ) -> Result<SearchPage> {
        let site_id = ctx.request().site_id()?;
        let user_id = ctx.request().user_id;
        self.search_with(site_id, request, |doc| async move {
            let Some(page) =
                PageService::get_direct_optional(ctx, doc.page_id, false).await?
            else {
                return Ok(None);
            };
            if page.site_id != site_id || page.latest_revision_id != Some(doc.revision_id)
            {
                return Ok(None);
            }
            let allowed = PermissionService::check_user_can(
                ctx,
                &CheckPermissionContext {
                    user_id,
                    site_id,
                    page_reference: Some(Reference::Id(doc.page_id)),
                },
                Permission {
                    resource_type: Resource::Page,
                    resource_category: Some(Reference::Id(page.page_category_id)),
                    action: Action::View,
                },
            )
            .await?;
            if !allowed {
                return Ok(None);
            }
            let Some(revision) = page_revision::Entity::find_by_id(doc.revision_id)
                .one(ctx.transaction())
                .await
                .or_raise(|| {
                    Error::new(
                        "failed to validate current search revision",
                        ErrorType::Page,
                    )
                })?
            else {
                return Ok(None);
            };
            if revision.page_id != page.page_id || revision.site_id != site_id {
                return Ok(None);
            }
            let html = TextService::get(ctx, &revision.compiled_body_html_hash).await?;
            let Some(mut hit) = SearchHit::from_current_document(&doc, &html) else {
                return Ok(None);
            };
            hit.title = revision.title;
            hit.tags = revision.tags;
            hit.slug = page.slug;
            Ok(Some(hit))
        })
        .await
    }

    async fn search_with<F, Fut>(
        &self,
        site_id: i64,
        input: SearchRequest,
        mut visible: F,
    ) -> Result<SearchPage>
    where
        F: FnMut(SearchDocument) -> Fut,
        Fut: Future<Output = Result<Option<SearchHit>>>,
    {
        if input.query.trim().is_empty()
            || input.query.len() > 200
            || input.limit == 0
            || input.limit > 20
            || input.offset > 500
        {
            return Err(Error::new(
                "invalid search query, offset or limit",
                ErrorType::BadRequest,
            )
            .into());
        }
        let mut raw_offset = 0;
        let mut visible_count = 0;
        let mut hits = Vec::new();
        loop {
            let size = BATCH.min(MAX_CANDIDATES - raw_offset);
            if size == 0 {
                return Err(Error::new(
                    "search exceeds candidate scan limit",
                    ErrorType::Request,
                )
                .into());
            }
            let batch: MeiliHits = self.request(Method::POST, &format!("/indexes/{INDEX}/search"), Some(serde_json::json!({
                "q": input.query, "filter": format!("site_id = {site_id}"), "offset": raw_offset, "limit": size,
            }))).await?;
            let count = batch.hits.len();
            for doc in batch.hits {
                if doc.site_id != site_id {
                    continue;
                }
                if let Some(hit) = visible(doc).await? {
                    if visible_count >= input.offset {
                        if hits.len() == input.limit {
                            return Ok(SearchPage {
                                hits,
                                has_more: true,
                            });
                        }
                        hits.push(hit);
                    }
                    visible_count += 1;
                }
            }
            raw_offset += count;
            if count < size {
                return Ok(SearchPage {
                    hits,
                    has_more: false,
                });
            }
        }
    }

    async fn wait_task(&self, uid: u64) -> Result<()> {
        for _ in 0..40 {
            let task: TaskStatus = self
                .request::<_, TaskStatus>(
                    Method::GET,
                    &format!("/tasks/{uid}"),
                    None::<()>,
                )
                .await?;
            match task.status.as_str() {
                "succeeded" => return Ok(()),
                "failed" | "canceled" => {
                    return Err(Error::new(
                        format!("Meilisearch task {uid} {}", task.status),
                        ErrorType::Request,
                    )
                    .into());
                }
                _ => tokio::time::sleep(Duration::from_millis(250)).await,
            }
        }
        Err(Error::new(
            format!("Meilisearch task {uid} timed out"),
            ErrorType::Request,
        )
        .into())
    }

    async fn request<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
    ) -> Result<T> {
        for attempt in 0..3 {
            let mut request = self
                .client
                .request(method.clone(), format!("{}{path}", self.url))
                .bearer_auth(&self.key);
            if let Some(ref body) = body {
                request = request.json(body);
            }
            match request.send().await {
                Ok(response) if response.status().is_success() => {
                    return response.json().await.or_raise(|| {
                        Error::new("invalid Meilisearch response", ErrorType::Request)
                    });
                }
                Ok(response) => {
                    let status = response.status();
                    if attempt < 2
                        && matches!(
                            status,
                            StatusCode::TOO_MANY_REQUESTS
                                | StatusCode::BAD_GATEWAY
                                | StatusCode::SERVICE_UNAVAILABLE
                                | StatusCode::GATEWAY_TIMEOUT
                        )
                    {
                        let delay = response
                            .headers()
                            .get(reqwest::header::RETRY_AFTER)
                            .and_then(|header| header.to_str().ok())
                            .and_then(|value| {
                                value
                                    .parse::<u64>()
                                    .ok()
                                    .map(Duration::from_secs)
                                    .or_else(|| {
                                        httpdate::parse_http_date(value).ok().map(
                                            |date| {
                                                date.duration_since(
                                                    std::time::SystemTime::now(),
                                                )
                                                .unwrap_or_default()
                                            },
                                        )
                                    })
                            })
                            .unwrap_or(Duration::from_millis(
                                100 * (1 << attempt) + rand::random_range(0..40),
                            ));
                        if delay > Duration::from_secs(5) {
                            return Err(Error::new(
                                "Meilisearch Retry-After exceeds retry budget",
                                ErrorType::Request,
                            )
                            .into());
                        }
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(Error::new(
                        format!("Meilisearch request failed: HTTP {status}"),
                        ErrorType::Request,
                    )
                    .into());
                }
                Err(error)
                    if attempt < 2 && (error.is_connect() || error.is_timeout()) =>
                {
                    tokio::time::sleep(Duration::from_millis(
                        100 * (1 << attempt) + rand::random_range(0..40),
                    ))
                    .await;
                }
                Err(error) => {
                    return Err(Error::new(
                        format!("Meilisearch unavailable: {error}"),
                        ErrorType::Request,
                    )
                    .into());
                }
            }
        }
        Err(Error::new("Meilisearch retry exhausted", ErrorType::Request).into())
    }
}

/// Extract text only from displayed HTML nodes. Compiled output, not raw source,
/// is required so conditional wikitext omitted during rendering cannot be indexed.
fn plain_body(html: &str) -> String {
    fn visit(node: scraper::ElementRef<'_>, output: &mut String) {
        for child in node.children() {
            match child.value() {
                Node::Text(text) => {
                    output.push_str(text);
                    output.push(' ');
                }
                Node::Element(element) => {
                    let name = element.name();
                    if ["script", "style", "template", "noscript", "svg"].contains(&name)
                        || element.attr("hidden").is_some()
                        || element.attr("class").is_some_and(|classes| {
                            classes.split_whitespace().any(|class| {
                                matches!(class, "wj-hidden" | "wj-invisible")
                            })
                        })
                        || element.attr("aria-hidden") == Some("true")
                        || element.attr("style").is_some_and(|style| {
                            style.replace(' ', "").contains("display:none")
                        })
                    {
                        continue;
                    }
                    if let Some(element) = scraper::ElementRef::wrap(child) {
                        visit(element, output);
                    }
                }
                _ => {}
            }
        }
    }
    let document = Html::parse_fragment(html);
    let mut text = String::new();
    visit(document.root_element(), &mut text);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod freshness_tests;
#[cfg(test)]
mod tests;
