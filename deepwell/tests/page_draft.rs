#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::ErrorType;
use deepwell::services::RequestContext;
use deepwell::types::Reference;
use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};
use serde_json::json;

const SLUG: &str = "draft:example";

async fn setup() -> (TestRunner, i64) {
    let mut runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    target(&mut runner, site_id, SLUG, Some(ADMIN_USER_ID));
    (runner, site_id)
}

fn target(runner: &mut TestRunner, site_id: i64, slug: &str, user_id: Option<i64>) {
    runner.set_request_context(RequestContext {
        user_id,
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug(slug.to_owned().into())),
        ..Default::default()
    });
}

async fn create(runner: &mut TestRunner, site_id: i64, slug: &str, source: &str) -> i64 {
    target(runner, site_id, slug, Some(ADMIN_USER_ID));
    run_endpoint!(
        runner,
        page_create,
        json!({
            "site_id": site_id, "slug": slug, "title": "Published",
            "wikitext": source, "revision_comments": "Fixture",
            "user_id": ADMIN_USER_ID, "ip_address": common::IP_ADDRESS
        })
    )
    .revision_id
}

async fn counts(runner: &TestRunner) -> Vec<i64> {
    let db = runner.context().transaction();
    let mut values = Vec::new();
    for table in [
        "page",
        "page_revision",
        "text",
        "text_block",
        "search_index_pending",
    ] {
        let row = db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("SELECT count(*) AS n FROM {table}"),
            ))
            .await
            .unwrap()
            .unwrap();
        values.push(row.try_get("", "n").unwrap());
    }
    values
}

#[tokio::test]
async fn missing_page_draft_round_trip_replaces_and_deletes_without_publishing() {
    let (runner, _) = setup().await;
    let before = counts(&runner).await;
    assert!(run_endpoint!(runner, page_draft_get).draft.is_none());
    let draft = run_endpoint!(
        runner,
        page_draft_save,
        json!({
            "title": "Raw 🦊", "wikitext": "line one\n[[code]]\nline two\n"
        })
    )
    .draft
    .unwrap();
    assert_eq!(draft.title, "Raw 🦊");
    assert_eq!(draft.wikitext, "line one\n[[code]]\nline two\n");
    let read = run_endpoint!(runner, page_draft_get).draft.unwrap();
    assert_eq!(read.title, draft.title);
    assert_eq!(read.wikitext, draft.wikitext);
    let replaced = run_endpoint!(
        runner,
        page_draft_save,
        json!({
            "title": "Replaced", "wikitext": "new\n"
        })
    )
    .draft
    .unwrap();
    assert_eq!(replaced.title, "Replaced");
    assert_eq!(replaced.wikitext, "new\n");
    assert_eq!(counts(&runner).await, before);
    run_endpoint!(runner, page_draft_delete);
    assert!(run_endpoint!(runner, page_draft_get).draft.is_none());
    assert_eq!(counts(&runner).await, before);
}

#[tokio::test]
async fn existing_page_requires_current_published_base_and_preserves_public_page() {
    let (mut runner, site_id) = setup().await;
    let revision_id = create(&mut runner, site_id, SLUG, "Public body").await;
    let before = counts(&runner).await;
    let error = run_endpoint_err!(
        runner,
        page_draft_save,
        json!({
            "title": "Draft", "wikitext": "Private body",
            "last_revision_id": revision_id - 1
        })
    );
    assert_contains_error!(error, ErrorType::NotLatestRevisionId);
    let error = run_endpoint_err!(
        runner,
        page_draft_save,
        json!({
            "title": "Draft", "wikitext": "Private body"
        })
    );
    assert_contains_error!(error, ErrorType::BadRequest);
    let saved = run_endpoint!(
        runner,
        page_draft_save,
        json!({
            "title": "Draft", "wikitext": "Private body",
            "last_revision_id": revision_id
        })
    )
    .draft
    .unwrap();
    assert_eq!(saved.wikitext, "Private body");
    let page = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": SLUG, "details": {"wikitext": true}
        })
    )
    .unwrap();
    assert_eq!(page.revision_id, revision_id);
    assert_eq!(page.wikitext.as_deref(), Some("Public body"));
    assert_eq!(counts(&runner).await, before);
}

#[tokio::test]
async fn anonymous_cannot_read_replace_or_delete_shared_draft_even_with_spoofed_body() {
    let (mut runner, site_id) = setup().await;
    run_endpoint!(
        runner,
        page_draft_save,
        json!({"title": "Secret", "wikitext": "Secret body"})
    );
    target(&mut runner, site_id, SLUG, None);
    let error =
        run_endpoint_err!(runner, page_draft_get, json!({"user_id": ADMIN_USER_ID}));
    assert_contains_error!(error, ErrorType::PermissionDenied);
    let error = run_endpoint_err!(
        runner,
        page_draft_save,
        json!({
            "title": "Spoof", "wikitext": "Spoof", "user_id": ADMIN_USER_ID,
            "site_id": site_id
        })
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    let error =
        run_endpoint_err!(runner, page_draft_delete, json!({"user_id": ADMIN_USER_ID}));
    assert_contains_error!(error, ErrorType::PermissionDenied);
    target(&mut runner, site_id, SLUG, Some(ADMIN_USER_ID));
    assert_eq!(
        run_endpoint!(runner, page_draft_get).draft.unwrap().title,
        "Secret"
    );
}

#[tokio::test]
async fn publication_cleanup_helper_removes_only_matching_site_and_slug() {
    let (mut runner, site_id) = setup().await;
    run_endpoint!(
        runner,
        page_draft_save,
        json!({"title": "First", "wikitext": "one"})
    );
    target(&mut runner, site_id, "draft:other", Some(ADMIN_USER_ID));
    run_endpoint!(
        runner,
        page_draft_save,
        json!({"title": "Other", "wikitext": "two"})
    );
    deepwell::services::page_draft::PageDraftService::delete_for_target(
        runner.context().transaction(),
        site_id,
        SLUG,
    )
    .await
    .unwrap();
    target(&mut runner, site_id, SLUG, Some(ADMIN_USER_ID));
    assert!(run_endpoint!(runner, page_draft_get).draft.is_none());
    target(&mut runner, site_id, "draft:other", Some(ADMIN_USER_ID));
    assert_eq!(
        run_endpoint!(runner, page_draft_get)
            .draft
            .unwrap()
            .wikitext,
        "two"
    );
}

#[tokio::test]
async fn create_publishes_and_discards_target_draft() {
    let (mut runner, site_id) = setup().await;
    run_endpoint!(
        runner,
        page_draft_save,
        json!({"title": "Unpublished", "wikitext": "Draft body"})
    );
    create(&mut runner, site_id, SLUG, "Published body").await;
    assert!(run_endpoint!(runner, page_draft_get).draft.is_none());
    let published = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": SLUG, "details": {"wikitext": true}
        })
    )
    .unwrap();
    assert_eq!(published.wikitext.as_deref(), Some("Published body"));
}

#[tokio::test]
async fn successful_edit_and_noop_save_original_both_discard_draft() {
    let (mut runner, site_id) = setup().await;
    let revision_id = create(&mut runner, site_id, SLUG, "Original body").await;
    run_endpoint!(
        runner,
        page_draft_save,
        json!({
            "title": "Unpublished", "wikitext": "Draft body", "last_revision_id": revision_id
        })
    );
    let edit = run_endpoint!(
        runner,
        page_edit,
        json!({
            "site_id": site_id, "page": SLUG, "last_revision_id": revision_id,
            "revision_comments": "Publish", "user_id": ADMIN_USER_ID,
            "wikitext": "New published body", "ip_address": common::IP_ADDRESS
        })
    )
    .unwrap();
    assert!(run_endpoint!(runner, page_draft_get).draft.is_none());
    run_endpoint!(
        runner,
        page_draft_save,
        json!({
            "title": "Another draft", "wikitext": "Another body",
            "last_revision_id": edit.revision_id
        })
    );
    let no_revision = run_endpoint!(
        runner,
        page_edit,
        json!({
            "site_id": site_id, "page": SLUG, "last_revision_id": edit.revision_id,
            "revision_comments": "Save original", "user_id": ADMIN_USER_ID,
            "wikitext": "New published body", "ip_address": common::IP_ADDRESS
        })
    );
    assert!(no_revision.is_none());
    assert!(run_endpoint!(runner, page_draft_get).draft.is_none());
}

#[tokio::test]
async fn rejected_stale_edit_keeps_draft() {
    let (mut runner, site_id) = setup().await;
    let revision_id = create(&mut runner, site_id, SLUG, "Original").await;
    run_endpoint!(
        runner,
        page_draft_save,
        json!({
            "title": "Unpublished", "wikitext": "Draft body", "last_revision_id": revision_id
        })
    );
    let error = run_endpoint_err!(
        runner,
        page_edit,
        json!({
            "site_id": site_id, "page": SLUG, "last_revision_id": revision_id - 1,
            "revision_comments": "Stale", "user_id": ADMIN_USER_ID,
            "wikitext": "Other", "ip_address": common::IP_ADDRESS
        })
    );
    assert_contains_error!(error, ErrorType::NotLatestRevisionId);
    assert_eq!(
        run_endpoint!(runner, page_draft_get)
            .draft
            .unwrap()
            .wikitext,
        "Draft body"
    );
}

#[tokio::test]
async fn rolling_back_nested_publication_restores_draft_and_published_page() {
    let (mut runner, site_id) = setup().await;
    let revision_id = create(&mut runner, site_id, SLUG, "Original").await;
    run_endpoint!(
        runner,
        page_draft_save,
        json!({
            "title": "Unpublished", "wikitext": "Draft body", "last_revision_id": revision_id
        })
    );
    let nested = runner.context().transaction().begin().await.unwrap();
    let ctx = deepwell::services::ServiceContext::new(runner.state(), &nested)
        .with_request(runner.context().request().clone());
    deepwell::endpoints::all::page_edit(
        &ctx,
        common::make_params(json!({
            "site_id": site_id, "page": SLUG, "last_revision_id": revision_id,
            "revision_comments": "Publish", "user_id": ADMIN_USER_ID,
            "wikitext": "Published change", "ip_address": common::IP_ADDRESS
        })),
    )
    .await
    .unwrap();
    assert!(
        deepwell::endpoints::all::page_draft_get(&ctx, common::empty_params())
            .await
            .unwrap()
            .draft
            .is_none()
    );
    nested.rollback().await.unwrap();
    assert_eq!(
        run_endpoint!(runner, page_draft_get)
            .draft
            .unwrap()
            .wikitext,
        "Draft body"
    );
    let page = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": SLUG, "details": {"wikitext": true}
        })
    )
    .unwrap();
    assert_eq!(page.revision_id, revision_id);
    assert_eq!(page.wikitext.as_deref(), Some("Original"));
}

#[tokio::test]
async fn structured_updates_preserve_unknown_typed_fields_and_return_source() {
    let (mut runner, site_id) = setup().await;
    create(
        &mut runner,
        site_id,
        "draft:_template",
        "[[form]]\nfields:\n  name:\n    type: text\n[[/form]]",
    )
    .await;
    let revision_id = create(
        &mut runner,
        site_id,
        SLUG,
        "name: Before\ncount: 7\nunknown: true\n",
    )
    .await;
    target(&mut runner, site_id, SLUG, Some(ADMIN_USER_ID));
    let before = counts(&runner).await;
    let saved = run_endpoint!(
        runner,
        page_draft_save,
        json!({
            "title": "Structured", "form_updates": {"name": "After"},
            "last_revision_id": revision_id
        })
    )
    .draft
    .unwrap();
    let values = wikidot_forms::parse_values(&saved.wikitext).unwrap();
    assert_eq!(
        serde_json::to_value(values).unwrap(),
        json!({"name": "After", "count": 7, "unknown": true})
    );
    let restored = run_endpoint!(runner, page_draft_get).draft.unwrap();
    assert_eq!(restored.wikitext, saved.wikitext);
    assert_eq!(
        serde_json::to_value(restored.form_values.unwrap()).unwrap(),
        json!({"name": "After", "count": 7, "unknown": true})
    );
    assert_eq!(counts(&runner).await, before);
}
