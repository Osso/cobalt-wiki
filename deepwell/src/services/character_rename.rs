//! Renames a character's short name everywhere it is used, as one background job.
//!
//! The short name is the page name in `character:<name>`/`bgc:<name>`. It is
//! also the character's tag, `re:<name>` mention tag, writing/arc/chain name
//! list token, `RelationshipBox` `pagename`, link target, and
//! `icons/icon_<name>.jpg`. The display name (title, form `name`) is unchanged.

use super::prelude::*;
use crate::api::ServerState;
use crate::error::exn_error_to_rpc_error;
use crate::services::file::{EditFile, EditFileBody};
use crate::services::job::Job;
use crate::services::page::{EditPageBody, MovePage};
use crate::services::site_tag_admin::{
    AffectedPage, edit_tags, get, lock_site_tags, name_list_template, query_error,
    rename_name_lists, replace_tag, require_editable, statement,
};
use crate::services::{
    FileService, JobService, OutdateService, PageService, RequestContext, ViewService,
};
use redis::AsyncCommands;
use regex::Regex;
use sea_orm::{ConnectionTrait, TransactionTrait};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::LazyLock;
use std::time::Duration;

const CHARACTER_CATEGORIES: [&str; 2] = ["character", "bgc"];
const ICON_PAGE: &str = "icons";

/// Lets the starting request commit before a worker reads its row.
const START_DELAY: Duration = Duration::from_secs(1);

/// How long a redelivered job waits before checking a running rename again.
pub const RUNNING_RECHECK_DELAY: Duration = Duration::from_secs(30);

/// Progress counters outlive any rename; the row holds the final outcome.
const PROGRESS_EXPIRY_SECS: u64 = 86_400;

/// A `RelationshipBox` include up to its first closing `]]`; its `pagename`
/// comes before any nested link in the relationship text.
static RELATIONSHIP_BOX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)\[\[\s*include\s+relationshipbox\b.*?\]\]").unwrap()
});

static SHORT_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[a-z0-9]+(-[a-z0-9]+)*$").unwrap());

#[derive(Debug, Deserialize)]
pub struct RenameCharacter {
    pub page_id: i64,
    pub new_name: String,
}

#[derive(Debug, Deserialize)]
pub struct StartCharacterRename {
    pub page_id: i64,
    pub new_name: String,
    pub preview_token: String,
    pub ip_address: IpAddr,
}

#[derive(Debug, Deserialize)]
pub struct GetCharacterRename {
    pub operation_id: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterRenamePreview {
    pub old_name: String,
    pub new_name: String,
    pub old_slug: String,
    pub new_slug: String,
    pub preview_token: String,
    pub icon: bool,
    pub count: usize,
    pub pages: Vec<AffectedPage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterRenameStarted {
    pub operation_id: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterRenameStatus {
    pub operation_id: i64,
    pub status: String,
    pub done: i64,
    pub total: i64,
    pub error: Option<String>,
    pub new_slug: String,
}

#[derive(Debug)]
struct Plan {
    site_id: i64,
    character_id: i64,
    character_revision_id: i64,
    old_name: String,
    new_name: String,
    old_slug: String,
    new_slug: String,
    pages: Vec<PlannedPage>,
    icon: Option<IconFile>,
    preview_token: String,
}

#[derive(Debug)]
struct PlannedPage {
    page: AffectedPage,
    revision_id: i64,
    tags: Option<Vec<String>>,
    wikitext: Option<String>,
}

#[derive(Debug)]
struct IconFile {
    page_id: i64,
    category_id: i64,
    file_id: i64,
    revision_id: i64,
}

#[derive(Debug)]
struct Candidate {
    page: AffectedPage,
    revision_id: i64,
    tags: Vec<String>,
    source: String,
}

#[derive(Debug)]
pub struct CharacterRenameService;

impl CharacterRenameService {
    pub async fn preview(
        ctx: &ServiceContext<'_>,
        input: RenameCharacter,
    ) -> Result<CharacterRenamePreview> {
        let plan = read_plan(ctx, input.page_id, &input.new_name).await?;
        require_plan_editable(ctx, &plan).await?;
        Ok(CharacterRenamePreview {
            old_name: plan.old_name,
            new_name: plan.new_name,
            old_slug: plan.old_slug,
            new_slug: plan.new_slug,
            preview_token: plan.preview_token,
            icon: plan.icon.is_some(),
            count: plan.pages.len(),
            pages: plan.pages.into_iter().map(|p| p.page).collect(),
        })
    }

    /// Records the confirmed rename and queues the job that performs it.
    pub async fn start(
        ctx: &ServiceContext<'_>,
        input: StartCharacterRename,
    ) -> Result<CharacterRenameStarted> {
        let user_id = ctx.request().user_id()?;
        let plan = read_plan(ctx, input.page_id, &input.new_name).await?;
        if plan.preview_token != input.preview_token {
            bail!(Error::new(
                "affected pages differ from preview",
                ErrorType::TagPreviewStale
            ));
        }
        require_plan_editable(ctx, &plan).await?;
        let total = step_count(&plan);
        let row = ctx.transaction().query_one_raw(statement(
            "INSERT INTO character_rename (site_id, page_id, actor_user_id, old_name, new_name, new_slug, preview_token, ip_address, total) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING operation_id",
            [plan.site_id.into(), plan.character_id.into(), user_id.into(), plan.old_name.clone().into(), plan.new_name.clone().into(), plan.new_slug.clone().into(), plan.preview_token.clone().into(), input.ip_address.to_string().into(), total.into()],
        )).await.or_raise(query_error)?.ok_or_raise(|| Error::new("character rename insert returned no row", ErrorType::DatabaseQuery))?;
        let operation_id = get(&row, "operation_id")?;
        JobService::queue_job(
            ctx,
            &Job::CharacterRename { operation_id },
            Some(START_DELAY),
        )
        .await?;
        Ok(CharacterRenameStarted { operation_id })
    }

    pub async fn status(
        ctx: &ServiceContext<'_>,
        input: GetCharacterRename,
    ) -> Result<CharacterRenameStatus> {
        let site_id = ctx.request().site_id()?;
        let user_id = ctx.request().user_id()?;
        let row = ctx.transaction().query_one_raw(statement(
            "SELECT actor_user_id, status, total, error, new_slug FROM character_rename WHERE operation_id = $1 AND site_id = $2",
            [input.operation_id.into(), site_id.into()],
        )).await.or_raise(query_error)?.ok_or_raise(|| Error::new("character rename not found on this site", ErrorType::BadRequest))?;
        if get::<i64>(&row, "actor_user_id")? != user_id {
            bail!(Error::new(
                "only the renaming user can follow its progress",
                ErrorType::PermissionDenied
            ));
        }
        let stored: String = get(&row, "status")?;
        let total: i32 = get(&row, "total")?;
        let progress = read_progress(ctx, input.operation_id).await?;
        // A running job's row stays queued until it commits; progress shows it started.
        let (status, done) = match (stored.as_str(), progress) {
            ("done", _) => (stored, i64::from(total)),
            ("queued", Some(done)) => ("running".to_owned(), done),
            (_, progress) => (stored, progress.unwrap_or(0)),
        };
        Ok(CharacterRenameStatus {
            operation_id: input.operation_id,
            status,
            done,
            total: i64::from(total),
            error: get(&row, "error")?,
            new_slug: get(&row, "new_slug")?,
        })
    }

    /// Performs a recorded rename in `ctx`, whose request is the renaming user.
    /// Every change commits or rolls back with that transaction.
    pub async fn execute(ctx: &ServiceContext<'_>, operation_id: i64) -> Result<()> {
        let row = ctx.transaction().query_one_raw(statement(
            "SELECT site_id, page_id, actor_user_id, new_name, preview_token, ip_address FROM character_rename WHERE operation_id = $1",
            [operation_id.into()],
        )).await.or_raise(query_error)?.ok_or_raise(|| Error::new("character rename not found", ErrorType::BadRequest))?;
        let site_id: i64 = get(&row, "site_id")?;
        let user_id: i64 = get(&row, "actor_user_id")?;
        let new_name: String = get(&row, "new_name")?;
        let token: String = get(&row, "preview_token")?;
        let ip_address = get::<String>(&row, "ip_address")?
            .parse::<IpAddr>()
            .or_raise(query_error)?;
        lock_site_tags(ctx, site_id).await?;
        reset_progress(ctx, operation_id).await?;
        let plan = read_plan(ctx, get(&row, "page_id")?, &new_name).await?;
        if plan.preview_token != token {
            bail!(Error::new(
                "affected pages changed since the rename was confirmed",
                ErrorType::TagPreviewStale
            ));
        }
        require_plan_editable(ctx, &plan).await?;
        let comment = format!("Rename character {} to {}", plan.old_name, plan.new_name);
        OutdateService::outdate_once(ctx, site_id, async {
            let moved = PageService::r#move(
                ctx,
                MovePage {
                    fix_dependencies: Vec::new(),
                    site_id,
                    page: Reference::Id(plan.character_id),
                    new_slug: plan.new_slug.clone(),
                    last_revision_id: plan.character_revision_id,
                    revision_comments: comment.clone(),
                    user_id,
                    ip_address,
                },
            )
            .await?;
            record_progress(ctx, operation_id).await?;
            for planned in &plan.pages {
                let revision_id = if planned.page.page_id == plan.character_id {
                    moved.revision_id
                } else {
                    planned.revision_id
                };
                edit_tags(
                    ctx,
                    site_id,
                    user_id,
                    planned.page.page_id,
                    revision_id,
                    EditPageBody {
                        tags: planned.tags.clone().map_or(Maybe::Unset, Maybe::Set),
                        wikitext: planned
                            .wikitext
                            .clone()
                            .map_or(Maybe::Unset, Maybe::Set),
                        ..Default::default()
                    },
                    ip_address,
                    &comment,
                )
                .await?;
                record_progress(ctx, operation_id).await?;
            }
            if let Some(icon) = &plan.icon {
                rename_icon(ctx, &plan, icon, user_id, ip_address, &comment).await?;
                record_progress(ctx, operation_id).await?;
            }
            Ok(())
        })
        .await?;
        ctx.transaction()
            .execute_raw(statement(
                "UPDATE character_rename SET status = 'done', finished_at = now() WHERE operation_id = $1",
                [operation_id.into()],
            ))
            .await
            .or_raise(query_error)?;
        Ok(())
    }

    /// Job body. The row lock is taken in the rename's own transaction, so a
    /// worker that dies mid-run releases it. A redelivery while the rename
    /// still runs finds the row locked and returns `true`: the caller checks
    /// again later, so a job exists until the rename commits or fails.
    /// Jobs queued by the rename are sent only after it commits.
    pub async fn run_job(state: &ServerState, operation_id: i64) -> Result<bool> {
        let txn = state.database.begin().await.or_raise(query_error)?;
        let request = match claim(&txn, operation_id).await? {
            Claim::Run(request) => request,
            Claim::RunningElsewhere => {
                info!("Character rename {operation_id} is running elsewhere");
                txn.rollback().await.or_raise(query_error)?;
                return Ok(true);
            }
            Claim::Finished => {
                info!("Character rename {operation_id} already finished");
                txn.rollback().await.or_raise(query_error)?;
                return Ok(false);
            }
        };
        let ctx = ServiceContext::new(state, &txn).with_request(request);
        ctx.defer_jobs();
        let result = Self::execute(&ctx, operation_id).await;
        let jobs = ctx.take_deferred_jobs();
        drop(ctx);
        match result {
            Ok(()) => {
                txn.commit().await.or_raise(query_error)?;
                JobService::send_deferred_jobs(&mut state.rsmq.clone(), &jobs).await?;
                Ok(false)
            }
            Err(error) => {
                txn.rollback().await.or_raise(query_error)?;
                JobService::discard_deferred_jobs(&mut state.redis.clone(), &jobs)
                    .await?;
                let rpc = exn_error_to_rpc_error(error);
                let message = format!("[{}] {}", rpc.code(), rpc.message());
                warn!("Character rename {operation_id} failed: {message}");
                record_failure(state, operation_id, &message).await?;
                Ok(false)
            }
        }
    }
}

async fn read_plan(
    ctx: &ServiceContext<'_>,
    page_id: i64,
    new_name: &str,
) -> Result<Plan> {
    let site_id = ctx.request().site_id()?;
    let user_id = ctx.request().user_id()?;
    let character = PageService::get(ctx, site_id, Reference::Id(page_id)).await?;
    let (category, old_name) = character_name(&character.slug)?;
    PageService::require_page_edit(
        ctx,
        site_id,
        user_id,
        page_id,
        character.page_category_id,
    )
    .await?;
    let new_name = validate_new_name(new_name, old_name)?;
    let new_slug = format!("{category}:{new_name}");
    require_unused_name(ctx, site_id, &new_name).await?;
    let icon = read_icon(ctx, site_id, old_name, &new_name).await?;
    let candidates = read_candidates(ctx, site_id, old_name).await?;
    let (pages, templates) =
        plan_pages(ctx, site_id, candidates, old_name, &new_name).await?;
    let character_revision_id = character.latest_revision_id.ok_or_raise(|| {
        Error::new("character page has no revision", ErrorType::BadRequest)
    })?;
    let preview_token = preview_token(
        site_id,
        page_id,
        character_revision_id,
        &new_name,
        &pages,
        &templates,
        icon.as_ref(),
    );
    Ok(Plan {
        site_id,
        character_id: page_id,
        character_revision_id,
        old_name: old_name.to_owned(),
        new_name,
        old_slug: character.slug.clone(),
        new_slug,
        pages,
        icon,
        preview_token,
    })
}

fn character_name(slug: &str) -> Result<(&str, &str)> {
    slug.split_once(':')
        .filter(|(category, name)| {
            CHARACTER_CATEGORIES.contains(category) && !name.starts_with('_')
        })
        .ok_or_raise(|| {
            Error::new(
                "only character and bgc pages can be renamed this way",
                ErrorType::BadRequest,
            )
        })
}

fn validate_new_name(new_name: &str, old_name: &str) -> Result<String> {
    let new_name = new_name.trim().to_lowercase();
    if !SHORT_NAME.is_match(&new_name) {
        bail!(Error::new(
            "new name must be lowercase letters, digits and single hyphens",
            ErrorType::BadRequest
        ));
    }
    if new_name == old_name {
        bail!(Error::new(
            "new name is the current name",
            ErrorType::BadRequest
        ));
    }
    Ok(new_name)
}

async fn require_unused_name(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    new_name: &str,
) -> Result<()> {
    let slugs: Vec<String> = CHARACTER_CATEGORIES
        .iter()
        .map(|category| format!("{category}:{new_name}"))
        .collect();
    let tags = vec![new_name.to_owned(), format!("re:{new_name}")];
    let row = ctx.transaction().query_one_raw(statement(
        "SELECT EXISTS (SELECT 1 FROM page WHERE site_id = $1 AND deleted_at IS NULL AND slug = ANY($2)) AS slug_taken, (SELECT count(*) FROM page p JOIN page_revision r ON r.revision_id = p.latest_revision_id WHERE p.site_id = $1 AND p.deleted_at IS NULL AND r.tags && $3) AS tagged",
        [site_id.into(), slugs.into(), tags.into()],
    )).await.or_raise(query_error)?.ok_or_raise(query_error)?;
    if get::<bool>(&row, "slug_taken")? {
        bail!(Error::new(
            format!("a character page named {new_name} already exists"),
            ErrorType::PageSlugExists
        ));
    }
    let tagged: i64 = get(&row, "tagged")?;
    if tagged > 0 {
        bail!(Error::new(
            format!("{tagged} pages already use the tag {new_name} or re:{new_name}"),
            ErrorType::CharacterNameInUse
        ));
    }
    Ok(())
}

async fn read_icon(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    old_name: &str,
    new_name: &str,
) -> Result<Option<IconFile>> {
    let names = vec![icon_name(old_name), icon_name(new_name)];
    let rows = ctx.transaction().query_all_raw(statement(
        "SELECT f.name, f.file_id, p.page_id, p.page_category_id, (SELECT max(r.revision_id) FROM file_revision r WHERE r.file_id = f.file_id) AS revision_id FROM file f JOIN page p ON p.page_id = f.page_id WHERE p.site_id = $1 AND p.slug = $2 AND p.deleted_at IS NULL AND f.deleted_at IS NULL AND f.name = ANY($3)",
        [site_id.into(), ICON_PAGE.into(), names.into()],
    )).await.or_raise(query_error)?;
    let mut icon = None;
    for row in rows {
        if get::<String>(&row, "name")? == icon_name(new_name) {
            bail!(Error::new(
                format!("icon {} already exists", icon_name(new_name)),
                ErrorType::FileExists
            ));
        }
        icon = Some(IconFile {
            page_id: get(&row, "page_id")?,
            category_id: get(&row, "page_category_id")?,
            file_id: get(&row, "file_id")?,
            revision_id: get(&row, "revision_id")?,
        });
    }
    Ok(icon)
}

fn icon_name(name: &str) -> String {
    format!("icon_{name}.jpg")
}

/// Live pages tagged with the name, or whose source mentions it as a word.
async fn read_candidates(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    old_name: &str,
) -> Result<Vec<Candidate>> {
    let tags = vec![old_name.to_owned(), format!("re:{old_name}")];
    // The validated slug grammar has no regex metacharacters.
    let word = format!("\\m{old_name}\\M");
    let rows = ctx.transaction().query_all_raw(statement(
        "SELECT p.page_id, p.slug, r.title, r.revision_id, r.tags, t.contents FROM page p JOIN page_revision r ON r.revision_id = p.latest_revision_id JOIN text t ON t.hash = r.wikitext_hash WHERE p.site_id = $1 AND p.deleted_at IS NULL AND (r.tags && $2 OR t.contents ~* $3) ORDER BY p.page_id",
        [site_id.into(), tags.into(), word.into()],
    )).await.or_raise(query_error)?;
    rows.iter()
        .map(|row| {
            Ok(Candidate {
                page: AffectedPage {
                    page_id: get(row, "page_id")?,
                    slug: get(row, "slug")?,
                    title: get(row, "title")?,
                },
                revision_id: get(row, "revision_id")?,
                tags: get(row, "tags")?,
                source: get(row, "contents")?,
            })
        })
        .collect()
}

async fn plan_pages(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    candidates: Vec<Candidate>,
    old_name: &str,
    new_name: &str,
) -> Result<(Vec<PlannedPage>, Vec<Option<String>>)> {
    let renames = [
        (old_name.to_owned(), new_name.to_owned()),
        (format!("re:{old_name}"), format!("re:{new_name}")),
    ];
    let mut templates: HashMap<&str, Option<String>> = HashMap::new();
    let mut pages = Vec::new();
    for candidate in candidates {
        let renamed_tags = rename_tags(&candidate.tags, &renames);
        let mut source = candidate.source.clone();
        if let Some(template_slug) = name_list_template(&candidate.page.slug) {
            if !templates.contains_key(template_slug) {
                let template = ViewService::load_visible_template_source(
                    ctx,
                    site_id,
                    ctx.request().user_id,
                    template_slug,
                )
                .await?;
                templates.insert(template_slug, template);
            }
            if let Some(template) = templates[template_slug].as_deref() {
                for (old, new) in &renames {
                    source = rename_name_lists(template, &source, old, new)
                        .or_raise(|| {
                            Error::new(
                                format!(
                                    "failed to rename name lists on {}",
                                    candidate.page.slug
                                ),
                                ErrorType::BadRequest,
                            )
                        })?
                        .unwrap_or(source);
                }
            }
        }
        let source = rewrite_references(&source, old_name, new_name);
        let tags = (renamed_tags != candidate.tags).then_some(renamed_tags);
        let wikitext = (source != candidate.source).then_some(source);
        if tags.is_some() || wikitext.is_some() {
            pages.push(PlannedPage {
                page: candidate.page,
                revision_id: candidate.revision_id,
                tags,
                wikitext,
            });
        }
    }
    // Sorted so the preview token does not depend on page order.
    let mut used: Vec<_> = templates.into_iter().collect();
    used.sort_unstable_by_key(|(slug, _)| *slug);
    Ok((pages, used.into_iter().map(|(_, source)| source).collect()))
}

fn rename_tags(tags: &[String], renames: &[(String, String)]) -> Vec<String> {
    renames.iter().fold(tags.to_vec(), |tags, (old, new)| {
        replace_tag(&tags, old, Some(new))
    })
}

/// Rewrites `pagename=<name>` inside `RelationshipBox` includes and
/// `[[[character:<name>…]]]`/`[[[bgc:<name>…]]]` link targets. Prose, other
/// includes' `pagename`, and other words containing the name stay unchanged.
fn rewrite_references(source: &str, old_name: &str, new_name: &str) -> String {
    let old_name = regex::escape(old_name);
    let replace = |pattern: String, text: &str| -> String {
        Regex::new(&pattern)
            .expect("valid reference pattern")
            .replace_all(text, |captures: &regex::Captures| {
                format!("{}{new_name}{}", &captures[1], &captures[2])
            })
            .into_owned()
    };
    let linked = replace(
        format!(r"(?i)(\[\[\[\s*(?:character|bgc):){old_name}([\s|\]#]|$)"),
        source,
    );
    RELATIONSHIP_BOX
        .replace_all(&linked, |include: &regex::Captures| {
            replace(
                format!(r"(?i)(pagename\s*=\s*){old_name}([\s|\]]|$)"),
                &include[0],
            )
        })
        .into_owned()
}

fn preview_token(
    site_id: i64,
    page_id: i64,
    revision_id: i64,
    new_name: &str,
    pages: &[PlannedPage],
    templates: &[Option<String>],
    icon: Option<&IconFile>,
) -> String {
    let mut digest = Sha256::new();
    digest.update(
        serde_json::to_vec(&(
            site_id,
            page_id,
            revision_id,
            new_name,
            pages
                .iter()
                .map(|p| (p.page.page_id, p.revision_id))
                .collect::<Vec<_>>(),
            icon.map(|icon| (icon.file_id, icon.revision_id)),
            templates,
        ))
        .expect("serializable character rename preview"),
    );
    hex::encode(digest.finalize())
}

async fn require_plan_editable(ctx: &ServiceContext<'_>, plan: &Plan) -> Result<()> {
    let user_id = ctx.request().user_id()?;
    for planned in &plan.pages {
        require_editable(ctx, plan.site_id, user_id, planned.page.page_id).await?;
    }
    if let Some(icon) = &plan.icon {
        PageService::require_page_edit(
            ctx,
            plan.site_id,
            user_id,
            icon.page_id,
            icon.category_id,
        )
        .await
        .or_raise(|| {
            Error::new(
                "cannot rename the character icon",
                ErrorType::TagPageNotEditable,
            )
        })?;
    }
    Ok(())
}

/// The move, each page edit, and the icon rename each count as one step.
fn step_count(plan: &Plan) -> i32 {
    let steps = 1 + plan.pages.len() + usize::from(plan.icon.is_some());
    i32::try_from(steps).expect("character rename step count fits i32")
}

async fn rename_icon(
    ctx: &ServiceContext<'_>,
    plan: &Plan,
    icon: &IconFile,
    user_id: i64,
    ip_address: IpAddr,
    comment: &str,
) -> Result<()> {
    FileService::edit(
        ctx,
        EditFile {
            site_id: plan.site_id,
            page_id: icon.page_id,
            file_id: icon.file_id,
            user_id,
            last_revision_id: icon.revision_id,
            revision_comments: comment.to_owned(),
            body: EditFileBody {
                name: Maybe::Set(icon_name(&plan.new_name)),
                ..Default::default()
            },
            bypass_filter: false,
            ip_address,
        },
    )
    .await?;
    Ok(())
}

fn progress_key(operation_id: i64) -> String {
    format!("character-rename:progress:{operation_id}")
}

async fn record_progress(ctx: &ServiceContext<'_>, operation_id: i64) -> Result<()> {
    let key = progress_key(operation_id);
    let mut redis = ctx.redis();
    let make_error =
        || Error::new("failed to record character rename progress", ErrorType::Job);
    let _: i64 = redis.incr(&key, 1).await.or_raise(make_error)?;
    let _: bool = redis
        .expire(&key, PROGRESS_EXPIRY_SECS as i64)
        .await
        .or_raise(make_error)?;
    Ok(())
}

/// A retried job starts counting again from zero.
async fn reset_progress(ctx: &ServiceContext<'_>, operation_id: i64) -> Result<()> {
    let _: () = ctx
        .redis()
        .set_ex(progress_key(operation_id), 0, PROGRESS_EXPIRY_SECS)
        .await
        .or_raise(|| {
            Error::new("failed to reset character rename progress", ErrorType::Job)
        })?;
    Ok(())
}

/// `None` until a job starts running the rename.
async fn read_progress(
    ctx: &ServiceContext<'_>,
    operation_id: i64,
) -> Result<Option<i64>> {
    ctx.redis()
        .get(progress_key(operation_id))
        .await
        .or_raise(|| {
            Error::new("failed to read character rename progress", ErrorType::Job)
        })
}

enum Claim {
    /// Locked for this transaction, to run as this user and page.
    Run(RequestContext),
    /// Queued, but another worker's transaction holds it.
    RunningElsewhere,
    Finished,
}

/// Locks a queued rename for this transaction.
async fn claim(txn: &sea_orm::DatabaseTransaction, operation_id: i64) -> Result<Claim> {
    let row = txn.query_one_raw(statement(
        "SELECT site_id, page_id, actor_user_id FROM character_rename WHERE operation_id = $1 AND status = 'queued' FOR UPDATE SKIP LOCKED",
        [operation_id.into()],
    )).await.or_raise(query_error)?;
    let Some(row) = row else {
        let status = txn
            .query_one_raw(statement(
                "SELECT status FROM character_rename WHERE operation_id = $1",
                [operation_id.into()],
            ))
            .await
            .or_raise(query_error)?;
        let Some(status) = status else {
            // Not committed yet: fail so the queue delivers the job again.
            bail!(Error::new(
                format!("character rename {operation_id} not visible yet"),
                ErrorType::Job
            ));
        };
        return Ok(if get::<String>(&status, "status")? == "queued" {
            Claim::RunningElsewhere
        } else {
            Claim::Finished
        });
    };
    let page_id: i64 = get(&row, "page_id")?;
    Ok(Claim::Run(RequestContext {
        session: None,
        user_id: Some(get(&row, "actor_user_id")?),
        site_id: Some(get(&row, "site_id")?),
        page_reference: Some(Reference::Id(page_id)),
    }))
}

async fn record_failure(
    state: &ServerState,
    operation_id: i64,
    message: &str,
) -> Result<()> {
    let txn = state.database.begin().await.or_raise(query_error)?;
    txn.execute_raw(statement(
        "UPDATE character_rename SET status = 'failed', error = $2, finished_at = now() WHERE operation_id = $1 AND status = 'queued'",
        [operation_id.into(), message.into()],
    ))
    .await
    .or_raise(query_error)?;
    txn.commit().await.or_raise(query_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_rename_only_page_names_and_link_targets() {
        let source = "[[include RelationshipBox | pagename=Dara | name=Dara]]\n\
            [[include RelationshipBox | pagename= dara|name=x]]\n\
            [[[character:dara|Dara]]] [[[bgc:dara]]] [[[character:dara#bio|b]]]\n\
            [[[character:darab|x]]] pagename=dara-x dara stays [[[writing:dara]]]\n\
            [[include PetBox | pagename=dara]]";
        assert_eq!(
            rewrite_references(source, "dara", "tennerow"),
            "[[include RelationshipBox | pagename=tennerow | name=Dara]]\n\
            [[include RelationshipBox | pagename= tennerow|name=x]]\n\
            [[[character:tennerow|Dara]]] [[[bgc:tennerow]]] [[[character:tennerow#bio|b]]]\n\
            [[[character:darab|x]]] pagename=dara-x dara stays [[[writing:dara]]]\n\
            [[include PetBox | pagename=dara]]"
        );
    }

    #[test]
    fn new_names_must_be_valid_tags_and_differ() {
        assert_eq!(validate_new_name(" Dara-Two ", "dara").unwrap(), "dara-two");
        for invalid in [
            "two words",
            "re:dara",
            "-dara",
            "dara-",
            "_dara",
            "",
            "dara",
        ] {
            assert!(validate_new_name(invalid, "dara").is_err(), "{invalid}");
        }
    }
}
