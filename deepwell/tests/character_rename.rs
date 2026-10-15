//! Character rename through registered RPCs: preview, start, the job body,
//! and status, against the native database, S3 and rendered pages.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::ErrorType;
use deepwell::services::blob::StartBlobUpload;
use deepwell::services::character_rename::CharacterRenameService;
use deepwell::services::job::{Job, JobService};
use deepwell::services::page_revision::RerenderType;
use deepwell::services::{BlobService, RequestContext, ServiceContext};
use deepwell::types::Reference;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};
use serde_json::json;

const WRITING_TEMPLATE: &str = "[[include writingbox | characters=%%form_raw{characters}%%]]\n\n====\n\n[[form]]\nfields:\n  characters:\n    type: text\n  references:\n    type: text\n  content:\n    type: wiki\n[[/form]]";
const ARC_TEMPLATE: &str = "[[include writingbox | characters=%%form_raw{characters}%%]]\n\n====\n\n[[form]]\nfields:\n  characters:\n    type: text\n[[/form]]";
const WRITINGBOX: &str = "[[module ListPages category=\"character bgc\" tags=\"{$characters}\" separate=\"no\" order=\"name\"]]\n[[image icons/icon_%%name%%.jpg alt=\"%%title%%\"]]\n[[/module]]";
const FRIEND: &str = "[[include relationshipbox | pagename=Old | name=Old One]]\n[[include relationshipbox | pagename= keep | name=Keep]]\nSee [[[character:old|Old One]]]. old prose and oldest stay.";
const RELATIONSHIPBOX: &str =
    "[[image icons/icon_{$pagename}.jpg link=\"character:{$pagename}\"]]";

struct Fixture {
    site: i64,
    character: i64,
    writing: i64,
    arc: i64,
    friend: i64,
    prose: i64,
    icon: i64,
}

async fn setup() -> (TestRunner, Fixture) {
    let mut runner = TestRunner::setup_with_idle_job_workers().await;
    // The native fixture predates this table; create it inside the rollback-only
    // test transaction without changing the shared fixture.
    let installed = runner
        .context()
        .transaction()
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT to_regclass('character_rename') IS NOT NULL AS installed",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<bool>("", "installed")
        .unwrap();
    // Operation IDs restart in each rolled-back table, so earlier runs' progress
    // counters in the shared Redis database would match them.
    let mut redis = runner.state().redis.clone();
    let stale: Vec<String> = redis::cmd("KEYS")
        .arg("character-rename:progress:*")
        .query_async(&mut redis)
        .await
        .unwrap();
    if !stale.is_empty() {
        let _: () = redis::cmd("DEL")
            .arg(stale)
            .query_async(&mut redis)
            .await
            .unwrap();
    }
    if !installed {
        runner
            .context()
            .transaction()
            .execute_unprepared(include_str!(
                "../migrations/20261015000000_character_rename.sql"
            ))
            .await
            .unwrap();
    }
    let site = run_endpoint!(runner, site_get, json!({"site":"test"}))
        .unwrap()
        .site
        .site_id;
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        ..Default::default()
    });
    import(&runner, site, "writingbox", WRITINGBOX, &[]).await;
    import(&runner, site, "relationshipbox", RELATIONSHIPBOX, &[]).await;
    import(&runner, site, "writing:_template", WRITING_TEMPLATE, &[]).await;
    import(&runner, site, "arc:_template", ARC_TEMPLATE, &[]).await;
    import(&runner, site, "character:keep", "Keep", &["keep"]).await;
    let character = import(
        &runner,
        site,
        "character:old",
        "Old One's biography mentions old friends.",
        &["old", "_completed"],
    )
    .await;
    let writing = import(
        &runner,
        site,
        "writing:meeting",
        "characters: 'Old, keep'\nreferences: 're:old re:oldest'\ncontent: 'old stays here'\n",
        &["old", "keep", "re:old"],
    )
    .await;
    let arc = import(&runner, site, "arc:saga", "characters: 'keep old'\n", &[]).await;
    let friend = import(&runner, site, "character:friend", FRIEND, &["friend"]).await;
    let prose = import(
        &runner,
        site,
        "writing:prose",
        "characters: keep\ncontent: 'the old ways'\n",
        &["keep"],
    )
    .await;
    let icons = import(&runner, site, "icons", "[[gallery]]", &[]).await;
    let icon = upload(&mut runner, site, icons, "icon_old.jpg").await;
    (
        runner,
        Fixture {
            site,
            character,
            writing,
            arc,
            friend,
            prose,
            icon,
        },
    )
}

async fn import(
    runner: &TestRunner,
    site: i64,
    slug: &str,
    source: &str,
    tags: &[&str],
) -> i64 {
    run_endpoint!(runner, page_import, json!({
        "site_id":site, "user_id":ADMIN_USER_ID, "slug":slug, "title":slug,
        "wikitext":source, "alt_title":null, "layout":"wikidot", "tags":tags,
        "bypass_filter":true, "revision_comments":"fixture", "ip_address":common::IP_ADDRESS
    }))
    .page_id
}

/// Uploads real bytes so the icon is a servable file, as on `icons` in production.
async fn upload(runner: &mut TestRunner, site: i64, page: i64, name: &str) -> i64 {
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(page)),
        ..Default::default()
    });
    let bytes = b"\xff\xd8\xff\xe0\x00\x10JFIF\x00character rename icon\xff\xd9";
    let transaction = runner.state().database.begin().await.unwrap();
    let pending = BlobService::start_upload(
        &ServiceContext::new(runner.state(), &transaction),
        StartBlobUpload {
            user_id: ADMIN_USER_ID,
            blob_size: bytes.len().try_into().unwrap(),
        },
    )
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    let response = reqwest::Client::new()
        .put(&pending.presign_url)
        .body(bytes.to_vec())
        .send()
        .await
        .unwrap();
    assert!(
        response.status().is_success(),
        "S3 PUT {}",
        response.status()
    );
    let created = run_endpoint!(
        runner,
        file_create,
        json!({
            "site_id":site, "page_id":page, "name":name,
            "uploaded_blob_id":pending.pending_blob_id, "user_id":ADMIN_USER_ID,
            "revision_comments":"icon", "bypass_filter":true, "ip_address":common::IP_ADDRESS
        })
    );
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        ..Default::default()
    });
    created.file_id
}

async fn read(
    runner: &TestRunner,
    site: i64,
    page: i64,
) -> deepwell::services::page::GetPageOutput {
    run_endpoint!(
        runner,
        page_get,
        json!({"site_id":site, "page":page, "details":{"wikitext":true,"compiled":true}})
    )
    .unwrap()
}

fn field(source: &str, name: &str) -> String {
    let values = wikidot_forms::parse_values(source).unwrap();
    values.get(name).unwrap().as_str().unwrap().to_owned()
}

#[tokio::test]
async fn rename_moves_page_and_updates_tags_name_lists_references_links_and_icon() {
    let (runner, f) = setup().await;
    let before: Vec<_> = {
        let mut pages = Vec::new();
        for page in [f.character, f.writing, f.arc, f.friend, f.prose] {
            pages.push(read(&runner, f.site, page).await);
        }
        pages
    };

    let preview = run_endpoint!(
        runner,
        character_rename_preview,
        json!({"page_id":f.character, "new_name":" New "})
    );
    assert_eq!(
        (
            preview.old_name.as_str(),
            preview.new_name.as_str(),
            preview.new_slug.as_str()
        ),
        ("old", "new", "character:new")
    );
    assert!(preview.icon);
    let mut slugs: Vec<&str> = preview.pages.iter().map(|p| p.slug.as_str()).collect();
    slugs.sort_unstable();
    assert_eq!(
        slugs,
        [
            "arc:saga",
            "character:friend",
            "character:old",
            "writing:meeting"
        ]
    );

    let started = run_endpoint!(
        runner,
        character_rename_start,
        json!({"page_id":f.character, "new_name":"new", "preview_token":preview.preview_token, "ip_address":common::IP_ADDRESS})
    );
    let queued = run_endpoint!(
        runner,
        character_rename_status,
        json!({"operation_id":started.operation_id})
    );
    assert_eq!((queued.status.as_str(), queued.total), ("queued", 6));
    // The job body; the worker wraps it in its own transaction.
    CharacterRenameService::execute(runner.context(), started.operation_id)
        .await
        .expect("rename job");
    let done = run_endpoint!(
        runner,
        character_rename_status,
        json!({"operation_id":started.operation_id})
    );
    assert_eq!(
        (done.status.as_str(), done.done, done.total),
        ("done", 6, 6)
    );
    assert_eq!(done.new_slug, "character:new");

    let character = read(&runner, f.site, f.character).await;
    assert_eq!(character.slug, "character:new");
    assert_eq!(
        character.title, "character:old",
        "display name is unchanged"
    );
    assert_eq!(character.tags, ["new", "_completed"]);
    assert_eq!(character.wikitext, before[0].wikitext, "prose is unchanged");
    assert_eq!(character.revision_number, before[0].revision_number + 2);
    assert!(
        run_endpoint!(
            runner,
            page_get,
            json!({"site_id":f.site, "page":"character:old"})
        )
        .is_none()
    );

    let writing = read(&runner, f.site, f.writing).await;
    let source = writing.wikitext.as_deref().unwrap();
    assert_eq!(field(source, "characters"), "new, keep");
    assert_eq!(field(source, "references"), "re:new re:oldest");
    assert_eq!(field(source, "content"), "old stays here");
    assert_eq!(writing.tags, ["new", "keep", "re:new"]);
    assert_eq!(writing.revision_number, before[1].revision_number + 1);
    assert!(
        writing
            .compiled_body_html
            .unwrap()
            .contains("/-/file/icons/icon_new.jpg")
    );

    let arc = read(&runner, f.site, f.arc).await;
    assert_eq!(
        field(arc.wikitext.as_deref().unwrap(), "characters"),
        "keep new"
    );
    assert_eq!(arc.revision_number, before[2].revision_number + 1);

    let friend = read(&runner, f.site, f.friend).await;
    assert_eq!(
        friend.wikitext.as_deref().unwrap(),
        "[[include relationshipbox | pagename=new | name=Old One]]\n[[include relationshipbox | pagename= keep | name=Keep]]\nSee [[[character:new|Old One]]]. old prose and oldest stay."
    );
    let html = friend.compiled_body_html.unwrap();
    assert!(html.contains("/-/file/icons/icon_new.jpg"), "{html}");
    assert!(html.contains("href=\"/character:new\""), "{html}");
    assert!(!html.contains("character:old"), "{html}");
    assert_eq!(friend.revision_number, before[3].revision_number + 1);

    let prose = read(&runner, f.site, f.prose).await;
    assert_eq!(
        prose.revision_id, before[4].revision_id,
        "prose-only page untouched"
    );

    let icons =
        run_endpoint!(runner, page_get, json!({"site_id":f.site, "page":"icons"}))
            .unwrap();
    let icon = run_endpoint!(
        runner,
        file_get,
        json!({"site_id":f.site, "page_id":icons.page_id, "file":"icon_new.jpg"})
    )
    .expect("renamed icon");
    assert_eq!(icon.file_id, f.icon);
    assert!(
        run_endpoint!(
            runner,
            file_get,
            json!({"site_id":f.site, "page_id":icons.page_id, "file":"icon_old.jpg"})
        )
        .is_none()
    );
}

#[tokio::test]
async fn conflicts_and_invalid_names_are_rejected_before_any_change() {
    let (runner, f) = setup().await;
    import(&runner, f.site, "bgc:taken", "Taken", &[]).await;
    import(
        &runner,
        f.site,
        "writing:mentions-used",
        "content: x\n",
        &["re:used"],
    )
    .await;
    let cases = [
        (f.character, "taken", ErrorType::PageSlugExists),
        (f.character, "used", ErrorType::CharacterNameInUse),
        (f.character, "two words", ErrorType::BadRequest),
        (f.character, "old", ErrorType::BadRequest),
        (f.writing, "fresh", ErrorType::BadRequest),
    ];
    for (page, name, expected) in cases {
        let error = run_endpoint_err!(
            runner,
            character_rename_preview,
            json!({"page_id":page, "new_name":name})
        );
        assert_contains_error!(error, found if *found == expected);
    }
}

#[tokio::test]
async fn existing_icon_name_is_rejected() {
    let (mut runner, f) = setup().await;
    let icons =
        run_endpoint!(runner, page_get, json!({"site_id":f.site, "page":"icons"}))
            .unwrap();
    upload(&mut runner, f.site, icons.page_id, "icon_fresh.jpg").await;
    let error = run_endpoint_err!(
        runner,
        character_rename_preview,
        json!({"page_id":f.character, "new_name":"fresh"})
    );
    assert_contains_error!(error, ErrorType::FileExists);
}

#[tokio::test]
async fn locked_pages_and_stale_previews_change_nothing() {
    let (mut runner, f) = setup().await;
    runner.set_request_context(RequestContext {
        site_id: Some(f.site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(f.friend)),
        ..Default::default()
    });
    run_endpoint!(
        runner,
        page_lock_create,
        json!({"page":f.friend,"lock_type":"permission-only","expires_at":null,"reason":null,"override_existing":false,"ip_address":common::IP_ADDRESS})
    );
    let error = run_endpoint_err!(
        runner,
        character_rename_preview,
        json!({"page_id":f.character, "new_name":"new"})
    );
    assert_contains_error!(error, ErrorType::TagPageNotEditable);

    runner.set_request_context(RequestContext {
        site_id: Some(f.site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(f.character)),
        ..Default::default()
    });
    run_endpoint!(
        runner,
        page_lock_create,
        json!({"page":f.character,"lock_type":"permission-only","expires_at":null,"reason":null,"override_existing":false,"ip_address":common::IP_ADDRESS})
    );
    let error = run_endpoint_err!(
        runner,
        character_rename_preview,
        json!({"page_id":f.character, "new_name":"new"})
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
}

/// The writing still changes its tags either way, so only the template decides
/// whether its `references` are rewritten as the preview showed.
#[tokio::test]
async fn template_edits_after_preview_make_the_rename_stale() {
    let (mut runner, f) = setup().await;
    let preview = run_endpoint!(
        runner,
        character_rename_preview,
        json!({"page_id":f.character, "new_name":"new"})
    );
    let template = run_endpoint!(
        runner,
        page_get,
        json!({"site_id":f.site, "page":"writing:_template", "details":{}})
    )
    .unwrap();
    runner.set_request_context(RequestContext {
        site_id: Some(f.site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(template.page_id)),
        ..Default::default()
    });
    let without_references =
        WRITING_TEMPLATE.replace("  references:\n    type: text\n", "");
    run_endpoint!(
        runner,
        page_edit,
        json!({"site_id":f.site,"page":template.page_id,"user_id":ADMIN_USER_ID,"last_revision_id":template.revision_id,"wikitext":without_references,"revision_comments":"drop references","ip_address":common::IP_ADDRESS})
    );
    let error = run_endpoint_err!(
        runner,
        character_rename_start,
        json!({"page_id":f.character, "new_name":"new", "preview_token":preview.preview_token, "ip_address":common::IP_ADDRESS})
    );
    assert_contains_error!(error, ErrorType::TagPreviewStale);
}

#[tokio::test]
async fn edits_after_preview_or_start_make_the_rename_stale() {
    let (mut runner, f) = setup().await;
    let preview = run_endpoint!(
        runner,
        character_rename_preview,
        json!({"page_id":f.character, "new_name":"new"})
    );
    let edit = |runner: &mut TestRunner, revision: i64, text: &str| {
        runner.set_request_context(RequestContext {
            site_id: Some(f.site),
            user_id: Some(ADMIN_USER_ID),
            page_reference: Some(Reference::Id(f.arc)),
            ..Default::default()
        });
        json!({"site_id":f.site,"page":f.arc,"user_id":ADMIN_USER_ID,"last_revision_id":revision,"wikitext":text,"revision_comments":"concurrent","ip_address":common::IP_ADDRESS})
    };
    let arc = read(&runner, f.site, f.arc).await;
    let params = edit(&mut runner, arc.revision_id, "characters: 'old keep '\n");
    run_endpoint!(runner, page_edit, params);
    let error = run_endpoint_err!(
        runner,
        character_rename_start,
        json!({"page_id":f.character, "new_name":"new", "preview_token":preview.preview_token, "ip_address":common::IP_ADDRESS})
    );
    assert_contains_error!(error, ErrorType::TagPreviewStale);

    let preview = run_endpoint!(
        runner,
        character_rename_preview,
        json!({"page_id":f.character, "new_name":"new"})
    );
    let started = run_endpoint!(
        runner,
        character_rename_start,
        json!({"page_id":f.character, "new_name":"new", "preview_token":preview.preview_token, "ip_address":common::IP_ADDRESS})
    );
    let arc = read(&runner, f.site, f.arc).await;
    let params = edit(&mut runner, arc.revision_id, "characters: 'old  keep'\n");
    run_endpoint!(runner, page_edit, params);
    let error = CharacterRenameService::execute(runner.context(), started.operation_id)
        .await
        .expect_err("stale job");
    assert_contains_error!(error, ErrorType::TagPreviewStale);
    assert_eq!(
        read(&runner, f.site, f.character).await.slug,
        "character:old"
    );
}

/// A rerender pending when the rename starts can begin while the rename runs
/// and read the old state, so pending rerenders are checked only after commit.
#[tokio::test]
async fn rerenders_pending_during_the_rename_are_queued_again_after_it() {
    let (runner, f) = setup().await;
    let preview = run_endpoint!(
        runner,
        character_rename_preview,
        json!({"page_id":f.character, "new_name":"new"})
    );
    let started = run_endpoint!(
        runner,
        character_rename_start,
        json!({"page_id":f.character, "new_name":"new", "preview_token":preview.preview_token, "ip_address":common::IP_ADDRESS})
    );
    let state = runner.state();
    let mut redis = state.redis.clone();
    let pending = format!("job:rerender-pending:{}:Standalone", f.writing);
    let _: () = redis::cmd("SET")
        .arg(&pending)
        .arg(1)
        .arg("EX")
        .arg(60)
        .query_async(&mut redis)
        .await
        .unwrap();

    runner.context().defer_jobs();
    CharacterRenameService::execute(runner.context(), started.operation_id)
        .await
        .expect("rename job");
    // The earlier job starts before the rename commits.
    let _: () = redis::cmd("DEL")
        .arg(&pending)
        .query_async(&mut redis)
        .await
        .unwrap();
    let jobs = runner.context().take_deferred_jobs();
    JobService::send_deferred_jobs(&mut redis, &mut state.rsmq.clone(), &jobs)
        .await
        .expect("send held jobs");

    let queued_again: bool = redis::cmd("EXISTS")
        .arg(&pending)
        .query_async(&mut redis)
        .await
        .unwrap();
    let _: () = redis::cmd("DEL")
        .arg(&pending)
        .query_async(&mut redis)
        .await
        .unwrap();
    assert!(
        jobs.iter().any(|(job, _)| matches!(
            job,
            Job::RerenderPage { id, r#type: RerenderType::Standalone } if id.page_id == f.writing
        )),
        "the writing's rerender must be held for after the rename"
    );
    assert!(
        queued_again,
        "the writing must be queued again after the rename"
    );
}

fn sql(query: &str) -> Statement {
    Statement::from_string(DatabaseBackend::Postgres, query)
}

/// A redelivered job must not drop the rename while another worker's
/// transaction holds it: if that worker dies, only a queued job can run it.
/// Runs against committed rows, as the worker does, and deletes its row.
#[tokio::test]
async fn redelivered_job_for_a_running_rename_checks_again_later() {
    let runner = TestRunner::setup_with_idle_job_workers().await;
    let state = runner.state();
    let database = &state.database;
    let installed = database
        .query_one_raw(sql(
            "SELECT to_regclass('character_rename') IS NOT NULL AS installed",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<bool>("", "installed")
        .unwrap();
    if !installed {
        database
            .execute_unprepared(include_str!(
                "../migrations/20261015000000_character_rename.sql"
            ))
            .await
            .unwrap();
    }
    let operation_id: i64 = database
        .query_one_raw(sql(&format!(
            "INSERT INTO character_rename (site_id, page_id, actor_user_id, old_name, new_name, new_slug, preview_token, ip_address, total)
             SELECT site_id, page_id, {ADMIN_USER_ID}, 'old', 'new', 'character:new', 'token', '127.0.0.1', 1
             FROM page WHERE deleted_at IS NULL ORDER BY page_id LIMIT 1
             RETURNING operation_id"
        )))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "operation_id")
        .unwrap();

    let running = database.begin().await.unwrap();
    running
        .execute_unprepared(&format!(
            "SELECT 1 FROM character_rename WHERE operation_id = {operation_id} FOR UPDATE"
        ))
        .await
        .unwrap();
    let running_elsewhere = CharacterRenameService::run_job(state, operation_id)
        .await
        .unwrap();
    running.rollback().await.unwrap();

    database
        .execute_unprepared(&format!(
            "UPDATE character_rename SET status = 'done' WHERE operation_id = {operation_id}"
        ))
        .await
        .unwrap();
    let after_done = CharacterRenameService::run_job(state, operation_id)
        .await
        .unwrap();
    database
        .execute_unprepared(&format!(
            "DELETE FROM character_rename WHERE operation_id = {operation_id}"
        ))
        .await
        .unwrap();

    assert!(running_elsewhere, "locked rename must be checked again");
    assert!(!after_done, "finished rename needs no further job");
}
