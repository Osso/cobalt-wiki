//! Preview is an authenticated, read-only render of submitted page content.
#[macro_use]
mod common;

use common::TestRunner;
use deepwell::config::Config;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::ErrorType;
use deepwell::services::RequestContext;
use deepwell::services::page::{CreatePageOutput, GetPageOutput};
use deepwell::types::Reference;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::json;
use std::time::Duration;
use tokio::time::timeout;

const SLUG: &str = "preview:example";
const TEMPLATE: &str = "[[form]]\nfields:\n  name:\n    type: text\n  count:\n    type: text\n[[/form]]\n+ %%title%%: %%form_data{name}%% (%%form_data{count}%%)";
const SOURCE: &str = "name: Before\ncount: 3\nunknown: true\n";

const PREPROCESS_BUDGET: Duration = Duration::from_millis(150);
const RENDER_BUDGET: Duration = Duration::from_millis(1000);
const INCLUDED_MARKER: &str = "INCLUDED_PREVIEW_DEPENDENCY_MARKER";

async fn setup() -> (TestRunner, i64) {
    let mut runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    target(
        &mut runner,
        site_id,
        Reference::Slug(SLUG.into()),
        Some(ADMIN_USER_ID),
    );
    (runner, site_id)
}

fn target(
    runner: &mut TestRunner,
    site_id: i64,
    page: Reference<'static>,
    user: Option<i64>,
) {
    runner.set_request_context(RequestContext {
        site_id: Some(site_id),
        page_reference: Some(page),
        user_id: user,
        ..Default::default()
    });
}

async fn setup_with_short_preprocess_budget() -> (TestRunner, i64) {
    let mut config = Config::integration_testing();
    config.preprocess_timeout = PREPROCESS_BUDGET;
    config.render_timeout = RENDER_BUDGET;
    let mut runner = TestRunner::setup_with_config(config).await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    target(
        &mut runner,
        site_id,
        Reference::Slug(SLUG.into()),
        Some(ADMIN_USER_ID),
    );
    (runner, site_id)
}

async fn delay_included_source_read(
    runner: &TestRunner,
    revision_id: i64,
    delay: Duration,
) {
    let db = runner.context().transaction();
    let revision = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            format!("SELECT wikitext_hash FROM page_revision WHERE revision_id = {revision_id}"),
        ))
        .await
        .unwrap()
        .unwrap();
    let hash: Vec<u8> = revision.try_get("", "wikitext_hash").unwrap();
    db.execute_unprepared(
        "CREATE FUNCTION pg_temp.delay_preview_text(value text, seconds double precision)
         RETURNS text LANGUAGE plpgsql AS $$
         BEGIN PERFORM pg_sleep(seconds); RETURN value; END $$",
    )
    .await
    .unwrap();
    let view = format!(
        "CREATE TEMP VIEW text AS SELECT hash,
         CASE WHEN hash = decode('{}', 'hex')
              THEN pg_temp.delay_preview_text(contents, {})
              ELSE contents END AS contents FROM public.text",
        hex::encode(hash),
        delay.as_secs_f64(),
    );
    db.execute_unprepared(&view).await.unwrap();
}

async fn create(
    runner: &mut TestRunner,
    site_id: i64,
    slug: &str,
    source: &str,
) -> CreatePageOutput {
    target(
        runner,
        site_id,
        Reference::Slug(slug.to_owned().into()),
        Some(ADMIN_USER_ID),
    );
    run_endpoint!(
        runner,
        page_create,
        json!({
            "site_id": site_id, "slug": slug, "title": "Stored title", "wikitext": source,
            "revision_comments": "Fixture", "user_id": ADMIN_USER_ID,
            "ip_address": common::IP_ADDRESS
        })
    )
}

async fn stored(runner: &TestRunner, site_id: i64, page_id: i64) -> GetPageOutput {
    run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": page_id, "details": {"wikitext": true}
        })
    )
    .unwrap()
}

async fn counts(runner: &TestRunner) -> Vec<i64> {
    let db = runner.context().transaction();
    let mut values = Vec::new();
    for table in [
        "page",
        "page_revision",
        "text",
        "text_block",
        "page_link",
        "search_index_pending",
    ] {
        let result = db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("SELECT count(*) AS n FROM {table}"),
            ))
            .await
            .unwrap()
            .unwrap();
        values.push(result.try_get("", "n").unwrap());
    }
    values
}

#[tokio::test]
async fn preview_renders_included_source_after_preprocess_budget_without_writes() {
    let (mut runner, site_id) = setup_with_short_preprocess_budget().await;
    let included =
        create(&mut runner, site_id, "preview:included", INCLUDED_MARKER).await;
    let page = create(&mut runner, site_id, SLUG, "Stored body").await;
    delay_included_source_read(&runner, included.revision_id, Duration::from_millis(500))
        .await;
    target(
        &mut runner,
        site_id,
        Reference::Id(page.page_id),
        Some(ADMIN_USER_ID),
    );
    let before = counts(&runner).await;
    let html = run_endpoint!(
        runner,
        page_preview,
        json!({
            "wikitext": "Submitted before\n[[include preview:included]]\nSubmitted after",
            "last_revision_id": page.revision_id
        })
    )
    .html;
    assert!(html.contains(INCLUDED_MARKER), "{html}");
    assert!(html.contains("Submitted after"), "{html}");
    assert!(!html.contains("Stored body"), "{html}");
    let saved = stored(&runner, site_id, page.page_id).await;
    assert_eq!(saved.revision_id, page.revision_id);
    assert_eq!(saved.wikitext.as_deref(), Some("Stored body"));
    assert_eq!(counts(&runner).await, before);
}

#[tokio::test]
async fn preview_over_aggregate_budget_fails_and_releases_database_transaction() {
    let (mut runner, site_id) = setup_with_short_preprocess_budget().await;
    let included =
        create(&mut runner, site_id, "preview:included", INCLUDED_MARKER).await;
    let page = create(&mut runner, site_id, SLUG, "Stored body").await;
    delay_included_source_read(
        &runner,
        included.revision_id,
        Duration::from_millis(1600),
    )
    .await;
    target(
        &mut runner,
        site_id,
        Reference::Id(page.page_id),
        Some(ADMIN_USER_ID),
    );
    let before = counts(&runner).await;
    let error = timeout(
        Duration::from_secs(4),
        deepwell::endpoints::all::page_preview(
            runner.context(),
            common::make_params(json!({
                "wikitext": "[[include preview:included]]",
                "last_revision_id": page.revision_id
            })),
        ),
    )
    .await
    .expect("preview must finish within a bounded interval")
    .expect_err("overbudget dependency read must fail");
    assert_contains_error!(error, ErrorType::RenderTimeout);
    let saved = timeout(
        Duration::from_secs(4),
        stored(&runner, site_id, page.page_id),
    )
    .await
    .expect("transaction must serve another page read after timeout");
    assert_eq!(saved.revision_id, page.revision_id);
    assert_eq!(saved.wikitext.as_deref(), Some("Stored body"));
    let after = timeout(Duration::from_secs(4), counts(&runner))
        .await
        .expect("transaction must serve another query after timeout");
    assert_eq!(after, before);
}

#[tokio::test]
async fn authorized_existing_and_missing_raw_preview_use_submitted_metadata_without_writes()
 {
    let (mut runner, site_id) = setup().await;
    let page = create(&mut runner, site_id, SLUG, "Stored body").await;
    target(
        &mut runner,
        site_id,
        Reference::Id(page.page_id),
        Some(ADMIN_USER_ID),
    );
    let before = counts(&runner).await;
    let html = run_endpoint!(
        runner,
        page_preview,
        json!({
            "title": "Preview title", "alt_title": "Alternate", "tags": ["preview"],
            "wikitext": "+ Submitted heading\nSubmitted body",
            "last_revision_id": page.revision_id
        })
    )
    .html;
    assert!(html.contains("Submitted heading"), "{html}");
    assert!(html.contains("Submitted body"), "{html}");
    assert!(!html.contains("Stored body"), "{html}");
    assert_eq!(
        stored(&runner, site_id, page.page_id)
            .await
            .wikitext
            .as_deref(),
        Some("Stored body")
    );
    assert_eq!(counts(&runner).await, before);

    target(
        &mut runner,
        site_id,
        Reference::Slug("preview:new".into()),
        Some(ADMIN_USER_ID),
    );
    let html = run_endpoint!(runner, page_preview, json!({
        "title": "New title", "tags": ["new"], "wikitext": "+ New heading\nMissing page preview"
    })).html;
    assert!(html.contains("New heading"), "{html}");
    assert!(html.contains("Missing page preview"), "{html}");
    assert_eq!(counts(&runner).await, before);
}

#[tokio::test]
async fn structured_preview_merges_form_values_and_rejects_stale_or_invalid_updates() {
    let (mut runner, site_id) = setup().await;
    create(&mut runner, site_id, "preview:_template", TEMPLATE).await;
    let page = create(&mut runner, site_id, SLUG, SOURCE).await;
    target(
        &mut runner,
        site_id,
        Reference::Id(page.page_id),
        Some(ADMIN_USER_ID),
    );
    let before = counts(&runner).await;
    let html = run_endpoint!(
        runner,
        page_preview,
        json!({
            "title": "Preview form", "last_revision_id": page.revision_id,
            "form_updates": {"name": "After", "count": 9}
        })
    )
    .html;
    assert!(html.contains("Preview form"), "{html}");
    assert!(html.contains("After"), "{html}");
    assert!(html.contains("9"), "{html}");
    assert!(!html.contains("Before"), "{html}");
    for request in [
        json!({"last_revision_id": page.revision_id, "form_updates": {"name": "After"}, "wikitext": ""}),
        json!({"last_revision_id": page.revision_id, "form_updates": {"unknown": "overwrite"}}),
        json!({"last_revision_id": page.revision_id, "form_updates": {"name": ["invalid"]}}),
        json!({"form_updates": {"name": "After"}}),
    ] {
        let error = run_endpoint_err!(runner, page_preview, request);
        assert_contains_error!(error, ErrorType::BadRequest);
    }
    let error = run_endpoint_err!(
        runner,
        page_preview,
        json!({
            "form_updates": {"name": "After"}, "last_revision_id": page.revision_id - 1
        })
    );
    assert_contains_error!(error, ErrorType::NotLatestRevisionId);
    let saved = stored(&runner, site_id, page.page_id).await;
    assert_eq!(saved.revision_id, page.revision_id);
    assert_eq!(saved.wikitext.as_deref(), Some(SOURCE));
    assert_eq!(counts(&runner).await, before);
}

#[tokio::test]
async fn trusted_request_identity_rejects_spoof_and_denied_access_before_source_validation()
 {
    let (mut runner, site_id) = setup().await;
    let page = create(&mut runner, site_id, SLUG, "Private body").await;
    let before = counts(&runner).await;
    target(&mut runner, site_id, Reference::Id(page.page_id), None);
    let error = run_endpoint_err!(
        runner,
        page_preview,
        json!({
            "site_id": site_id, "page": page.page_id, "user_id": ADMIN_USER_ID,
            "wikitext": "", "form_updates": {}
        })
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_no_error!(error, ErrorType::BadRequest);
    target(
        &mut runner,
        site_id,
        Reference::Slug("preview:denied".into()),
        None,
    );
    let error = run_endpoint_err!(
        runner,
        page_preview,
        json!({
            "site_id": site_id, "user_id": ADMIN_USER_ID, "title": "Spoof", "wikitext": "spoof"
        })
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    target(
        &mut runner,
        site_id,
        Reference::Id(page.page_id),
        Some(ADMIN_USER_ID),
    );
    let html = run_endpoint!(
        runner,
        page_preview,
        json!({
            "site_id": site_id + 1, "page": "preview:other", "user_id": -1,
            "wikitext": "Trusted target"
        })
    )
    .html;
    assert!(html.contains("Trusted target"), "{html}");
    assert_eq!(counts(&runner).await, before);
}
