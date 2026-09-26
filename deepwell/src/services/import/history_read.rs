//! Authorized read-only source and comparison for one history origin.

use super::ImportedHistoryService;
use super::history::authorize_read;
use super::history_listing_structs::HistoryOrigin;
use super::history_structs::ReadImportedRevision;
use crate::error::prelude::*;
use crate::models::page_revision::Model as PageRevision;
use crate::services::score::{ScoreService, ScoreValue};
use crate::services::{
    PageRevisionService, RenderService, ServiceContext, SettingsService, SiteService,
    TextService, UserService,
};
use crate::types::Reference;
use ftml::data::PageInfo;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::borrow::Cow;

#[derive(Debug)]
pub struct HistoryReadService;

#[derive(Debug, Deserialize)]
pub struct ReadHistoryRevision {
    pub site_id: i64,
    pub page_id: i64,
    pub origin: HistoryOrigin,
    pub number: i32,
    #[serde(default)]
    pub rendered: bool,
}

#[derive(Debug, Deserialize)]
pub struct ReadHistoryCompare {
    pub site_id: i64,
    pub page_id: i64,
    pub origin: HistoryOrigin,
    pub from: i32,
    pub to: i32,
}

#[derive(Debug, Serialize)]
pub struct HistoryRevisionOutput {
    pub id: i64,
    pub number: i32,
    pub source: String,
    pub rendered_html: Option<String>,
    pub representation: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HistoryCompareOutput {
    pub from: i32,
    pub to: i32,
    pub lines: Vec<HistoryDiffLine>,
    pub representation: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HistoryDiffLine {
    pub kind: &'static str,
    pub text: String,
}

struct SelectedRevision {
    output: HistoryRevisionOutput,
    metadata: PageRevision,
}

fn invalid(message: &str) -> ExnError {
    Error::new(message, ErrorType::DatabaseImport).into()
}

impl HistoryReadService {
    pub async fn revision(
        ctx: &ServiceContext<'_>,
        input: ReadHistoryRevision,
    ) -> Result<Option<HistoryRevisionOutput>> {
        if input.number < 0 {
            return Err(invalid("history revision number must be nonnegative"));
        }
        authorize_read(ctx, input.site_id, input.page_id).await?;
        let Some(mut selected) = load_revision(
            ctx,
            input.site_id,
            input.page_id,
            input.origin,
            input.number,
        )
        .await?
        else {
            return Ok(None);
        };
        if input.rendered {
            selected.output.rendered_html = Some(
                render_selected(ctx, input.site_id, input.page_id, &selected).await?,
            );
        }
        Ok(Some(selected.output))
    }

    pub async fn compare(
        ctx: &ServiceContext<'_>,
        input: ReadHistoryCompare,
    ) -> Result<HistoryCompareOutput> {
        if input.from < 0 || input.to < 0 || input.from == input.to {
            return Err(invalid(
                "history comparison requires two distinct nonnegative revision numbers",
            ));
        }
        authorize_read(ctx, input.site_id, input.page_id).await?;
        let from =
            load_revision(ctx, input.site_id, input.page_id, input.origin, input.from)
                .await?
                .ok_or_else(|| invalid("history comparison from revision is missing"))?;
        let to = load_revision(ctx, input.site_id, input.page_id, input.origin, input.to)
            .await?
            .ok_or_else(|| invalid("history comparison to revision is missing"))?;
        let lines = TextDiff::from_lines(&from.output.source, &to.output.source)
            .iter_all_changes()
            .map(|change| HistoryDiffLine {
                kind: match change.tag() {
                    ChangeTag::Equal => "same",
                    ChangeTag::Delete => "delete",
                    ChangeTag::Insert => "insert",
                },
                text: change.value().to_owned(),
            })
            .collect();
        Ok(HistoryCompareOutput {
            from: input.from,
            to: input.to,
            lines,
            representation: from.output.representation,
        })
    }
}

async fn load_revision(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    page_id: i64,
    origin: HistoryOrigin,
    number: i32,
) -> Result<Option<SelectedRevision>> {
    match origin {
        HistoryOrigin::Wikidot => load_imported(ctx, site_id, page_id, number).await,
        HistoryOrigin::Local => load_local(ctx, site_id, page_id, number).await,
    }
}

async fn load_imported(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    page_id: i64,
    number: i32,
) -> Result<Option<SelectedRevision>> {
    let Some(revision) = ImportedHistoryService::source(
        ctx,
        ReadImportedRevision {
            site_id,
            page_id,
            source_revision_number: number,
        },
    )
    .await?
    else {
        return Ok(None);
    };
    // Use captured metadata when present; absent fields remain current render context.
    let mut current = PageRevisionService::get_latest(ctx, site_id, page_id).await?;
    if let Some(title) = revision.metadata.source_title {
        current.title = title;
    }
    if let Some(slug) = revision.metadata.source_slug {
        current.slug = slug;
    }
    if let Some(tags) = revision.metadata.source_tags {
        current.tags = tags;
    }
    Ok(Some(SelectedRevision {
        output: HistoryRevisionOutput {
            id: revision.metadata.source_revision_id,
            number,
            source: revision.wikitext,
            rendered_html: None,
            representation: Some(revision.metadata.representation),
        },
        metadata: current,
    }))
}

async fn load_local(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    page_id: i64,
    number: i32,
) -> Result<Option<SelectedRevision>> {
    let Some(revision) =
        PageRevisionService::get_optional(ctx, site_id, page_id, number).await?
    else {
        return Ok(None);
    };
    if revision
        .hidden
        .iter()
        .any(|field| field == "wikitext" || field == "compiled")
    {
        return Err(
            Error::new("historical source is hidden", ErrorType::Permission).into(),
        );
    }
    let source = TextService::get(ctx, &revision.wikitext_hash).await?;
    Ok(Some(SelectedRevision {
        output: HistoryRevisionOutput {
            id: revision.revision_id,
            number,
            source,
            rendered_html: None,
            representation: None,
        },
        metadata: revision,
    }))
}

async fn render_selected(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    page_id: i64,
    selected: &SelectedRevision,
) -> Result<String> {
    let site = SiteService::get(ctx, Reference::Id(site_id)).await?;
    let layout = SettingsService::get_layout(ctx, site_id, Some(page_id)).await?;
    let score = ScoreService::score(ctx, page_id).await?;
    let info = revision_page_info(&selected.metadata, &site.slug, &site.locale, score);
    let viewer = match ctx.request().user_id {
        Some(id) => {
            UserService::get_real_optional(ctx, Reference::Id(id))
                .await?
                .ok_or_else(|| {
                    Error::new("history viewer no longer exists", ErrorType::Permission)
                })?
                .slug
        }
        None => String::new(),
    };
    RenderService::render_page_for_viewer(
        ctx,
        selected.output.source.clone(),
        &info,
        layout,
        &viewer,
    )
    .await
}

fn revision_page_info<'a>(
    revision: &'a PageRevision,
    site: &'a str,
    language: &'a str,
    score: ScoreValue,
) -> PageInfo<'a> {
    let (category, page) = crate::utils::split_category(&revision.slug);
    PageInfo {
        page: Cow::Borrowed(page),
        category: category.map(Cow::Borrowed),
        site: Cow::Borrowed(site),
        title: Cow::Borrowed(&revision.title),
        alt_title: revision.alt_title.as_deref().map(Cow::Borrowed),
        score,
        tags: revision
            .tags
            .iter()
            .map(|tag| Cow::Borrowed(tag.as_str()))
            .collect(),
        language: Cow::Borrowed(language),
    }
}
