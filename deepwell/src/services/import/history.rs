use super::history_structs::*;
use super::prelude::*;
use crate::models::imported_page_revision::{self, Entity as History, Model};
use crate::models::page::{self, Entity as Page};
use crate::services::permission::{CheckPermissionContext, PermissionService};
use crate::services::{PageRevisionService, PageService, TextService};
use crate::types::{Action, Permission, Reference, Resource};
use sea_orm::IntoActiveModel;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct ImportedHistoryService;

fn invalid(message: &str) -> ExnError {
    Error::new(message, ErrorType::DatabaseImport).into()
}

impl ImportedHistoryService {
    pub async fn import(
        ctx: &ServiceContext<'_>,
        input: ImportHistory,
    ) -> Result<ImportHistoryOutput> {
        validate_input(&input)?;
        lock_current_page(ctx, &input).await?;
        let existing = History::find()
            .filter(imported_page_revision::Column::SiteId.eq(input.site_id))
            .filter(imported_page_revision::Column::PageId.eq(input.page_id))
            .all(ctx.transaction())
            .await
            .or_raise(|| {
                Error::new(
                    "failed to read existing imported history",
                    ErrorType::DatabaseImport,
                )
            })?;
        if existing
            .iter()
            .any(|row| row.source_page_id != input.source_page_id)
        {
            return Err(invalid(
                "page already has a different source history identity",
            ));
        }
        let by_id: HashMap<_, _> = existing
            .iter()
            .map(|row| (row.source_revision_id, row))
            .collect();
        let by_number: HashMap<_, _> = existing
            .iter()
            .map(|row| (row.source_revision_number, row))
            .collect();
        let mut inserted = 0;
        for revision in input.revisions {
            let row = build_row(
                ctx,
                input.site_id,
                input.page_id,
                input.source_page_id,
                revision,
            )
            .await?;
            if let Some(prior) = by_id
                .get(&row.source_revision_id)
                .or_else(|| by_number.get(&row.source_revision_number))
            {
                if *prior != &row {
                    return Err(invalid(
                        "imported revision conflicts with the stored source record",
                    ));
                }
                continue;
            }
            row.into_active_model()
                .insert(ctx.transaction())
                .await
                .or_raise(|| {
                    Error::new(
                        "failed to store imported revision",
                        ErrorType::DatabaseImport,
                    )
                })?;
            inserted += 1;
        }
        Ok(ImportHistoryOutput { inserted })
    }

    pub async fn list(
        ctx: &ServiceContext<'_>,
        input: ReadImportedHistory,
    ) -> Result<Vec<ImportedRevisionSummary>> {
        if input.limit == 0 || input.limit > 100 {
            return Err(invalid("history page limit must be between 1 and 100"));
        }
        authorize_read(ctx, input.site_id, input.page_id, input.user_id).await?;
        let mut query = History::find()
            .filter(imported_page_revision::Column::SiteId.eq(input.site_id))
            .filter(imported_page_revision::Column::PageId.eq(input.page_id));
        if let Some(before) = input.before_revision {
            query = query
                .filter(imported_page_revision::Column::SourceRevisionNumber.lt(before));
        }
        use imported_page_revision::Column as HistoryColumn;
        let rows = query
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
            .limit(input.limit)
            .into_model::<ImportedRevisionSummary>()
            .all(ctx.transaction())
            .await
            .or_raise(|| {
                Error::new(
                    "failed to list imported revisions",
                    ErrorType::DatabaseImport,
                )
            })?;
        Ok(rows)
    }

    pub async fn source(
        ctx: &ServiceContext<'_>,
        input: ReadImportedRevision,
    ) -> Result<Option<ImportedRevisionSource>> {
        authorize_read(ctx, input.site_id, input.page_id, input.user_id).await?;
        let row = History::find()
            .filter(imported_page_revision::Column::SiteId.eq(input.site_id))
            .filter(imported_page_revision::Column::PageId.eq(input.page_id))
            .filter(
                imported_page_revision::Column::SourceRevisionNumber
                    .eq(input.source_revision_number),
            )
            .one(ctx.transaction())
            .await
            .or_raise(|| {
                Error::new(
                    "failed to read imported revision",
                    ErrorType::DatabaseImport,
                )
            })?;
        let Some(row) = row else {
            return Ok(None);
        };
        let wikitext = TextService::get(ctx, &row.wikitext_hash).await?;
        Ok(Some(ImportedRevisionSource {
            metadata: row.into(),
            wikitext,
        }))
    }
}

fn validate_input(input: &ImportHistory) -> Result<()> {
    if input.source_page_id <= 0 || input.revisions.is_empty() {
        return Err(invalid("source page identity and revisions are required"));
    }
    let mut ids = HashSet::new();
    let mut numbers = HashSet::new();
    for row in &input.revisions {
        if row.source_revision_id <= 0
            || row.source_revision_number < 0
            || row.source_author_id.is_some_and(|id| id <= 0)
        {
            return Err(invalid("invalid source revision identity"));
        }
        if !ids.insert(row.source_revision_id)
            || !numbers.insert(row.source_revision_number)
        {
            return Err(invalid("duplicate source revision identity in request"));
        }
        if row.representation != "display-decoded-not-byte-exact"
            || row.raw_source_html.is_empty()
        {
            return Err(invalid(
                "history requires its raw source display and explicit representation",
            ));
        }
    }
    Ok(())
}

async fn lock_current_page(
    ctx: &ServiceContext<'_>,
    input: &ImportHistory,
) -> Result<()> {
    let page = Page::find()
        .filter(page::Column::PageId.eq(input.page_id))
        .filter(page::Column::SiteId.eq(input.site_id))
        .filter(page::Column::DeletedAt.is_null())
        .lock_exclusive()
        .one(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new(
                "failed to lock history target page",
                ErrorType::DatabaseImport,
            )
        })?;
    if page.is_none() {
        return Err(invalid("history target page does not exist on this site"));
    }
    let current =
        PageRevisionService::get_latest(ctx, input.site_id, input.page_id).await?;
    if current.revision_id != input.expected_revision_id {
        return Err(invalid(
            "current page revision changed before history import",
        ));
    }
    Ok(())
}

async fn build_row(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    page_id: i64,
    source_page_id: i64,
    revision: ImportHistoryRevision,
) -> Result<Model> {
    let hash = TextService::create(ctx, revision.wikitext).await?;
    Ok(Model {
        site_id,
        page_id,
        source_page_id,
        source_revision_id: revision.source_revision_id,
        source_revision_number: revision.source_revision_number,
        source_author_id: revision.source_author_id,
        source_created_at: revision.source_created_at,
        source_comments: revision.source_comments,
        source_flags: revision.source_flags,
        source_title: revision.source_title,
        source_slug: revision.source_slug,
        source_tags: revision.source_tags,
        wikitext_hash: hash.to_vec(),
        raw_source_html: revision.raw_source_html,
        acquired_at: revision.acquired_at,
        representation: revision.representation,
    })
}

async fn authorize_read(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    page_id: i64,
    user_id: Option<i64>,
) -> Result<()> {
    let page = PageService::get(ctx, site_id, Reference::Id(page_id)).await?;
    let allowed = PermissionService::check_user_can(
        ctx,
        &CheckPermissionContext {
            user_id,
            site_id,
            page_reference: Some(Reference::Id(page_id)),
        },
        Permission {
            resource_type: Resource::Page,
            resource_category: Some(Reference::Id(page.page_category_id)),
            action: Action::View,
        },
    )
    .await?;
    if !allowed {
        return Err(Error::new(
            "page history is not visible to this user",
            ErrorType::Permission,
        )
        .into());
    }
    Ok(())
}
