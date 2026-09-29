//! Site-wide tag changes use ordinary page revisions and one request transaction.

use super::prelude::*;
use crate::services::page::{EditPage, EditPageBody};
use crate::services::{
    MemberAdminService, OutdateService, PageRevisionService, PageService,
};
use sea_orm::{DatabaseBackend, Statement};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use time::OffsetDateTime;

#[derive(Debug, Deserialize, Clone)]
pub struct TagNames {
    pub source_tag: String,
    pub target_tag: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApplyTags {
    pub source_tag: String,
    pub target_tag: Option<String>,
    pub preview_token: String,
    pub ip_address: IpAddr,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AddFilters {
    pub existing_tag: Option<String>,
    pub category: Option<String>,
    pub title_contains: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddTagSelection {
    pub tag: String,
    #[serde(flatten)]
    pub filters: AddFilters,
}

#[derive(Debug, Deserialize)]
pub struct ApplyAddTag {
    pub tag: String,
    #[serde(flatten)]
    pub filters: AddFilters,
    pub preview_token: String,
    pub ip_address: IpAddr,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddTagPreview {
    pub tag: String,
    #[serde(flatten)]
    pub filters: AddFilters,
    pub preview_token: String,
    pub count: usize,
    pub pages: Vec<AffectedPage>,
}

#[derive(Debug, Deserialize)]
pub struct UndoTags {
    pub operation_id: i64,
    pub ip_address: IpAddr,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AffectedPage {
    pub page_id: i64,
    pub slug: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagPreview {
    pub source_tag: String,
    pub target_tag: Option<String>,
    pub preview_token: String,
    pub count: usize,
    pub pages: Vec<AffectedPage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagChangeResult {
    pub operation_id: i64,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagOperation {
    pub operation_id: i64,
    pub kind: String,
    pub source_tag: Option<String>,
    pub target_tag: Option<String>,
    pub filters: Option<AddFilters>,
    pub actor_user_id: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub count: usize,
    pub reversed: bool,
    pub pages: Vec<AffectedPage>,
}

#[derive(Debug)]
struct TaggedPage {
    page: AffectedPage,
    revision_id: i64,
    tags: Vec<String>,
}

#[derive(Debug)]
struct RecordedPage {
    page: AffectedPage,
    before_tags: Vec<String>,
    after_tags: Vec<String>,
    resulting_revision_id: i64,
}

#[derive(Debug)]
pub struct SiteTagAdminService;

impl SiteTagAdminService {
    pub async fn add_preview(
        ctx: &ServiceContext<'_>,
        selection: AddTagSelection,
    ) -> Result<AddTagPreview> {
        let (site_id, _) = require_admin(ctx).await?;
        let selection = normalize_add_selection(selection)?;
        let pages = add_pages(ctx, site_id, &selection).await?;
        Ok(build_add_preview(site_id, selection, &pages))
    }

    pub async fn add_apply(
        ctx: &ServiceContext<'_>,
        input: ApplyAddTag,
    ) -> Result<TagChangeResult> {
        let (site_id, user_id) = require_admin(ctx).await?;
        let selection = normalize_add_selection(AddTagSelection {
            tag: input.tag,
            filters: input.filters,
        })?;
        lock_site_tags(ctx, site_id).await?;
        let pages = add_pages(ctx, site_id, &selection).await?;
        let preview = build_add_preview(site_id, selection, &pages);
        if preview.preview_token != input.preview_token || pages.is_empty() {
            bail!(Error::new(
                "affected pages differ from preview",
                ErrorType::TagPreviewStale
            ));
        }
        for tagged in &pages {
            require_editable(ctx, site_id, user_id, tagged.page.page_id).await?;
        }
        let operation_id = insert_add_operation(ctx, site_id, user_id, &preview).await?;
        OutdateService::outdate_listings_once(ctx, site_id, async {
            for tagged in &pages {
                let mut after = tagged.tags.clone();
                after.push(preview.tag.clone());
                let revision_id = edit_tags(
                    ctx,
                    site_id,
                    user_id,
                    tagged.page.page_id,
                    tagged.revision_id,
                    after.clone(),
                    input.ip_address,
                    "Site tag add",
                )
                .await?;
                insert_record(ctx, operation_id, tagged, &after, revision_id).await?;
            }
            Ok(())
        })
        .await?;
        Ok(TagChangeResult {
            operation_id,
            count: pages.len(),
        })
    }

    pub async fn preview(
        ctx: &ServiceContext<'_>,
        names: TagNames,
    ) -> Result<TagPreview> {
        let (site_id, _) = require_admin(ctx).await?;
        let names = normalize_names(names)?;
        let pages = tagged_pages(ctx, site_id, &names.source_tag).await?;
        Ok(build_preview(site_id, names, &pages))
    }

    pub async fn apply(
        ctx: &ServiceContext<'_>,
        input: ApplyTags,
    ) -> Result<TagChangeResult> {
        let (site_id, user_id) = require_admin(ctx).await?;
        let names = normalize_names(TagNames {
            source_tag: input.source_tag,
            target_tag: input.target_tag,
        })?;
        lock_site_tags(ctx, site_id).await?;
        let pages = tagged_pages(ctx, site_id, &names.source_tag).await?;
        let preview = build_preview(site_id, names, &pages);
        if preview.preview_token != input.preview_token || pages.is_empty() {
            bail!(Error::new(
                "affected pages differ from preview",
                ErrorType::TagPreviewStale
            ));
        }
        let mut changed = Vec::with_capacity(pages.len());
        // Check all edit restrictions before the first write, so an error in this
        // phase never leaves changes in an externally managed transaction.
        for tagged in &pages {
            require_editable(ctx, site_id, user_id, tagged.page.page_id).await?;
            changed.push(replace_tag(
                &tagged.tags,
                &preview.source_tag,
                preview.target_tag.as_deref(),
            ));
        }
        let operation_id =
            insert_operation(ctx, site_id, user_id, &preview, input.ip_address).await?;
        OutdateService::outdate_listings_once(ctx, site_id, async {
            for (tagged, after) in pages.iter().zip(changed) {
                let revision_id = edit_tags(
                    ctx,
                    site_id,
                    user_id,
                    tagged.page.page_id,
                    tagged.revision_id,
                    after.clone(),
                    input.ip_address,
                    "Site tag change",
                )
                .await?;
                insert_record(ctx, operation_id, tagged, &after, revision_id).await?;
            }
            Ok(())
        })
        .await?;
        Ok(TagChangeResult {
            operation_id,
            count: pages.len(),
        })
    }

    pub async fn history(ctx: &ServiceContext<'_>) -> Result<Vec<TagOperation>> {
        let (site_id, _) = require_admin(ctx).await?;
        let rows = ctx.transaction().query_all_raw(statement(
            "SELECT operation_id, kind, source_tag, target_tag, filters, actor_user_id, created_at, reversed_at IS NOT NULL AS reversed FROM site_tag_operation WHERE site_id = $1 ORDER BY operation_id DESC",
            [site_id.into()],
        )).await.or_raise(query_error)?;
        let mut operations = Vec::with_capacity(rows.len());
        for row in rows {
            let operation_id = get(&row, "operation_id")?;
            let pages = recorded_pages(ctx, operation_id).await?;
            operations.push(TagOperation {
                operation_id,
                kind: get(&row, "kind")?,
                source_tag: get(&row, "source_tag")?,
                target_tag: get(&row, "target_tag")?,
                filters: get::<Option<serde_json::Value>>(&row, "filters")?
                    .map(serde_json::from_value)
                    .transpose()
                    .or_raise(query_error)?,
                actor_user_id: get(&row, "actor_user_id")?,
                created_at: get(&row, "created_at")?,
                count: pages.len(),
                reversed: get(&row, "reversed")?,
                pages: pages.into_iter().map(|p| p.page).collect(),
            });
        }
        Ok(operations)
    }

    pub async fn undo(
        ctx: &ServiceContext<'_>,
        input: UndoTags,
    ) -> Result<TagChangeResult> {
        let (site_id, user_id) = require_admin(ctx).await?;
        lock_site_tags(ctx, site_id).await?;
        let operation = ctx.transaction().query_one_raw(statement(
            "SELECT reversed_at FROM site_tag_operation WHERE site_id = $1 AND operation_id = $2 FOR UPDATE",
            [site_id.into(), input.operation_id.into()],
        )).await.or_raise(query_error)?.ok_or_raise(|| Error::new("tag operation not found on this site", ErrorType::BadRequest))?;
        let reversed_at: Option<OffsetDateTime> = get(&operation, "reversed_at")?;
        if reversed_at.is_some() {
            bail!(Error::new(
                "tag operation already reversed",
                ErrorType::BadRequest
            ));
        }
        let records = recorded_pages(ctx, input.operation_id).await?;
        for record in &records {
            let page =
                PageService::get(ctx, site_id, Reference::Id(record.page.page_id)).await;
            if !matches!(page, Ok(ref page) if page.latest_revision_id == Some(record.resulting_revision_id))
            {
                bail!(Error::new(
                    "page changed since tag operation",
                    ErrorType::TagUndoConflict
                ));
            }
            let revision =
                PageRevisionService::get_latest(ctx, site_id, record.page.page_id)
                    .await?;
            if revision.tags != record.after_tags {
                bail!(Error::new(
                    "page tags changed since tag operation",
                    ErrorType::TagUndoConflict
                ));
            }
            require_editable(ctx, site_id, user_id, record.page.page_id).await?;
        }
        OutdateService::outdate_listings_once(ctx, site_id, async {
            for record in &records {
                edit_tags(
                    ctx,
                    site_id,
                    user_id,
                    record.page.page_id,
                    record.resulting_revision_id,
                    record.before_tags.clone(),
                    input.ip_address,
                    "Undo site tag change",
                )
                .await?;
            }
            Ok(())
        })
        .await?;
        ctx.transaction().execute_raw(statement(
            "UPDATE site_tag_operation SET reversed_at = now() WHERE site_id = $1 AND operation_id = $2",
            [site_id.into(), input.operation_id.into()],
        )).await.or_raise(query_error)?;
        Ok(TagChangeResult {
            operation_id: input.operation_id,
            count: records.len(),
        })
    }
}

async fn require_admin(ctx: &ServiceContext<'_>) -> Result<(i64, i64)> {
    let request = ctx.request();
    let denied =
        || Error::new("only site admins manage tags", ErrorType::PermissionDenied);
    let Some(site_id) = request.site_id else {
        bail!(denied());
    };
    let Some(user_id) = request.user_id else {
        bail!(denied());
    };
    if request.session.as_ref().is_some_and(|s| s.restricted)
        || !MemberAdminService::is_site_admin(ctx, site_id, user_id).await?
    {
        bail!(denied());
    }
    Ok((site_id, user_id))
}

fn normalize_names(mut names: TagNames) -> Result<TagNames> {
    names.source_tag = names.source_tag.trim().to_lowercase();
    names.target_tag = names.target_tag.map(|target| target.trim().to_lowercase());
    if names.source_tag.is_empty()
        || names.source_tag.chars().any(char::is_whitespace)
        || names.target_tag.as_ref().is_some_and(|target| {
            target.is_empty()
                || target.chars().any(char::is_whitespace)
                || target == &names.source_tag
        })
    {
        bail!(Error::new(
            "invalid source or destination tag",
            ErrorType::BadRequest
        ));
    }
    Ok(names)
}

fn normalize_add_selection(mut selection: AddTagSelection) -> Result<AddTagSelection> {
    selection.tag = selection.tag.trim().to_lowercase();
    selection.filters.existing_tag = selection
        .filters
        .existing_tag
        .map(|s| s.trim().to_lowercase());
    selection.filters.category =
        selection.filters.category.map(|s| s.trim().to_lowercase());
    selection.filters.title_contains = selection
        .filters
        .title_contains
        .map(|s| s.trim().to_owned());
    selection.filters.title_contains =
        selection.filters.title_contains.filter(|s| !s.is_empty());
    let valid_tag = |tag: &str| !tag.is_empty() && !tag.chars().any(char::is_whitespace);
    let filters = &selection.filters;
    if !valid_tag(&selection.tag)
        || filters
            .existing_tag
            .as_deref()
            .is_some_and(|s| !valid_tag(s))
        || filters.category.as_deref().is_some_and(|s| !valid_tag(s))
        || filters.existing_tag.is_none()
            && filters.category.is_none()
            && filters.title_contains.is_none()
    {
        bail!(Error::new(
            "invalid tag or empty add filter",
            ErrorType::BadRequest
        ));
    }
    Ok(selection)
}

fn build_add_preview(
    site_id: i64,
    selection: AddTagSelection,
    tagged: &[TaggedPage],
) -> AddTagPreview {
    let mut digest = Sha256::new();
    digest.update(
        serde_json::to_vec(&(
            "add",
            site_id,
            &selection.tag,
            &selection.filters,
            tagged
                .iter()
                .map(|p| (&p.page, p.revision_id, &p.tags))
                .collect::<Vec<_>>(),
        ))
        .expect("serializable add preview"),
    );
    AddTagPreview {
        tag: selection.tag,
        filters: selection.filters,
        preview_token: hex::encode(digest.finalize()),
        count: tagged.len(),
        pages: tagged.iter().map(|p| p.page.clone()).collect(),
    }
}

async fn add_pages(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    selection: &AddTagSelection,
) -> Result<Vec<TaggedPage>> {
    let rows = ctx.transaction().query_all_raw(statement(
        "SELECT p.page_id, p.slug, r.title, r.revision_id, r.tags FROM page p JOIN page_revision r ON r.revision_id = p.latest_revision_id JOIN page_category c ON c.category_id = p.page_category_id WHERE p.site_id = $1 AND p.deleted_at IS NULL AND NOT ($2 = ANY(r.tags)) AND ($3::text IS NULL OR $3 = ANY(r.tags)) AND ($4::text IS NULL OR c.slug = $4) AND ($5::text IS NULL OR strpos(lower(r.title), lower($5)) > 0) ORDER BY p.page_id",
        [site_id.into(), selection.tag.clone().into(), selection.filters.existing_tag.clone().into(), selection.filters.category.clone().into(), selection.filters.title_contains.clone().into()],
    )).await.or_raise(query_error)?;
    read_tagged_pages(&rows)
}

fn replace_tag(tags: &[String], source: &str, target: Option<&str>) -> Vec<String> {
    let target_present =
        target.is_some_and(|target| tags.iter().any(|tag| tag == target));
    let mut inserted_target = false;
    let mut result = Vec::with_capacity(tags.len());
    for tag in tags {
        if target == Some(tag.as_str()) {
            if inserted_target {
                continue;
            }
            inserted_target = true;
        }
        if tag != source {
            result.push(tag.clone());
        } else if let Some(target) = target
            && !target_present
            && !inserted_target
        {
            result.push(target.to_owned());
            inserted_target = true;
        }
    }
    result
}

fn build_preview(site_id: i64, names: TagNames, tagged: &[TaggedPage]) -> TagPreview {
    let mut digest = Sha256::new();
    digest.update(
        serde_json::to_vec(&(
            site_id,
            &names.source_tag,
            &names.target_tag,
            tagged
                .iter()
                .map(|p| (&p.page, p.revision_id, &p.tags))
                .collect::<Vec<_>>(),
        ))
        .expect("serializable tag preview"),
    );
    TagPreview {
        source_tag: names.source_tag,
        target_tag: names.target_tag,
        preview_token: hex::encode(digest.finalize()),
        count: tagged.len(),
        pages: tagged.iter().map(|p| p.page.clone()).collect(),
    }
}

async fn tagged_pages(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    source: &str,
) -> Result<Vec<TaggedPage>> {
    let rows = ctx.transaction().query_all_raw(statement(
        "SELECT p.page_id, p.slug, r.title, r.revision_id, r.tags FROM page p JOIN page_revision r ON r.revision_id = p.latest_revision_id WHERE p.site_id = $1 AND p.deleted_at IS NULL AND $2 = ANY(r.tags) ORDER BY p.page_id",
        [site_id.into(), source.into()],
    )).await.or_raise(query_error)?;
    read_tagged_pages(&rows)
}

fn read_tagged_pages(rows: &[sea_orm::QueryResult]) -> Result<Vec<TaggedPage>> {
    rows.iter()
        .map(|row| {
            Ok(TaggedPage {
                page: AffectedPage {
                    page_id: get(row, "page_id")?,
                    slug: get(row, "slug")?,
                    title: get(row, "title")?,
                },
                revision_id: get(row, "revision_id")?,
                tags: get(row, "tags")?,
            })
        })
        .collect()
}

async fn recorded_pages(
    ctx: &ServiceContext<'_>,
    operation_id: i64,
) -> Result<Vec<RecordedPage>> {
    let rows = ctx.transaction().query_all_raw(statement(
        "SELECT page_id, slug, title, before_tags, after_tags, resulting_revision_id FROM site_tag_operation_page WHERE operation_id = $1 ORDER BY page_id",
        [operation_id.into()],
    )).await.or_raise(query_error)?;
    rows.iter()
        .map(|row| {
            Ok(RecordedPage {
                page: AffectedPage {
                    page_id: get(row, "page_id")?,
                    slug: get(row, "slug")?,
                    title: get(row, "title")?,
                },
                before_tags: get(row, "before_tags")?,
                after_tags: get(row, "after_tags")?,
                resulting_revision_id: get(row, "resulting_revision_id")?,
            })
        })
        .collect()
}

async fn require_editable(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    user_id: i64,
    page_id: i64,
) -> Result<()> {
    let page = PageService::get(ctx, site_id, Reference::Id(page_id)).await?;
    PageService::require_page_edit(ctx, site_id, user_id, page_id, page.page_category_id)
        .await
        .or_raise(|| {
            Error::new(
                format!("cannot edit affected page {}", page_id),
                ErrorType::TagPageNotEditable,
            )
        })
}

async fn edit_tags(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    user_id: i64,
    page_id: i64,
    revision_id: i64,
    tags: Vec<String>,
    ip_address: IpAddr,
    comments: &str,
) -> Result<i64> {
    let output = PageService::edit(
        ctx,
        EditPage {
            site_id,
            page: Reference::Id(page_id),
            last_revision_id: revision_id,
            revision_comments: comments.to_owned(),
            user_id,
            body: EditPageBody {
                tags: Maybe::Set(tags),
                preserve_draft: true,
                ..Default::default()
            },
            ip_address,
        },
    )
    .await?
    .ok_or_raise(|| {
        Error::new("tag edit produced no revision", ErrorType::TagPreviewStale)
    })?;
    Ok(output.revision_id)
}

async fn insert_operation(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    user_id: i64,
    preview: &TagPreview,
    _ip: IpAddr,
) -> Result<i64> {
    let row = ctx.transaction().query_one_raw(statement(
        "INSERT INTO site_tag_operation (site_id, actor_user_id, source_tag, target_tag, kind) VALUES ($1, $2, $3, $4, CASE WHEN $4::text IS NULL THEN 'delete' ELSE 'rename' END) RETURNING operation_id",
        [site_id.into(), user_id.into(), preview.source_tag.clone().into(), preview.target_tag.clone().into()],
    )).await.or_raise(query_error)?.ok_or_raise(|| Error::new("tag operation insert returned no row", ErrorType::DatabaseQuery))?;
    get(&row, "operation_id")
}

async fn insert_add_operation(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    user_id: i64,
    preview: &AddTagPreview,
) -> Result<i64> {
    let filters =
        serde_json::to_value(&preview.filters).expect("serializable add filters");
    let row = ctx.transaction().query_one_raw(statement(
        "INSERT INTO site_tag_operation (site_id, actor_user_id, kind, target_tag, filters) VALUES ($1, $2, 'add', $3, $4) RETURNING operation_id",
        [site_id.into(), user_id.into(), preview.tag.clone().into(), filters.into()],
    )).await.or_raise(query_error)?.ok_or_raise(|| Error::new("tag operation insert returned no row", ErrorType::DatabaseQuery))?;
    get(&row, "operation_id")
}

async fn insert_record(
    ctx: &ServiceContext<'_>,
    operation_id: i64,
    tagged: &TaggedPage,
    after: &[String],
    revision_id: i64,
) -> Result<()> {
    ctx.transaction().execute_raw(statement(
        "INSERT INTO site_tag_operation_page (operation_id, page_id, slug, title, before_tags, after_tags, resulting_revision_id) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        [operation_id.into(), tagged.page.page_id.into(), tagged.page.slug.clone().into(), tagged.page.title.clone().into(), tagged.tags.clone().into(), after.to_vec().into(), revision_id.into()],
    )).await.or_raise(query_error)?;
    Ok(())
}

pub(crate) async fn lock_site_tags(ctx: &ServiceContext<'_>, site_id: i64) -> Result<()> {
    ctx.transaction()
        .execute_raw(statement(
            "SELECT pg_advisory_xact_lock($1)",
            [site_id.into()],
        ))
        .await
        .or_raise(query_error)?;
    Ok(())
}

fn statement(sql: &str, values: impl IntoIterator<Item = sea_orm::Value>) -> Statement {
    Statement::from_sql_and_values(DatabaseBackend::Postgres, sql, values)
}

fn get<T: sea_orm::TryGetable>(row: &sea_orm::QueryResult, column: &str) -> Result<T> {
    row.try_get("", column).or_raise(query_error)
}

fn query_error() -> Error {
    Error::new(
        "failed to query site tag operation",
        ErrorType::DatabaseQuery,
    )
}
