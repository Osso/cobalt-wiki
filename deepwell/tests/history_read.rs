//! Read-only historical source, rendering, and source comparison.
#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::prelude::ErrorType;
use deepwell::models::{page_revision, role_permission};
use deepwell::services::TextService;
use deepwell::types::{Action, Resource};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel,
    QueryFilter,
};
use serde_json::{Value, json};

async fn fixture(runner: &TestRunner) -> (i64, i64, i64) {
    let site_id = run_endpoint!(runner, site_get, json!({"site":"test"}))
        .unwrap()
        .site
        .site_id;
    let created = run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id":site_id,"user_id":ADMIN_USER_ID,"slug":"history-read-fixture",
            "title":"Current title","wikitext":"Current source", "alt_title":null,
            "layout":"wikidot","revision_comments":"Technical import","bypass_filter":true,
            "ip_address":"127.0.0.1"
        })
    );
    let current = run_endpoint!(
        runner,
        page_get,
        json!({"site_id":site_id,"page":created.page_id})
    )
    .unwrap();
    (site_id, created.page_id, current.revision_id)
}

fn request(
    site_id: i64,
    page_id: i64,
    origin: &str,
    number: i32,
    rendered: bool,
) -> Value {
    json!({"site_id":site_id,"page_id":page_id,"origin":origin,"number":number,"rendered":rendered})
}

async fn imported(runner: &TestRunner, site_id: i64, page_id: i64, current: i64) {
    let sources = ["first\n旧\nlast\n", "first\n新\nlast\n"];
    let revisions: Vec<_> = sources.iter().enumerate().map(|(number, source)| json!({
        "source_revision_id": 8_200_000 + number,"source_revision_number":number,
        "source_author_id":null,"source_created_at":"2021-04-29T12:00:00Z",
        "source_comments":"", "source_flags":["S"],"source_title":null,
        "source_slug":null,"source_tags":null,"wikitext":source,
        "raw_source_html":"<pre>original display</pre>","acquired_at":"2026-09-23T00:00:00Z",
        "representation":"display-decoded-not-byte-exact"
    })).collect();
    run_endpoint!(
        runner,
        import_wikidot_history,
        json!({
            "site_id":site_id,"page_id":page_id,"source_page_id":1310927108,
            "expected_revision_id":current,"revisions":revisions
        })
    );
}

#[tokio::test]
async fn imported_revision_renders_selected_source_without_changing_page() {
    let runner = TestRunner::setup().await;
    let (site_id, page_id, current) = fixture(&runner).await;
    imported(&runner, site_id, page_id, current).await;
    let before = run_endpoint!(runner, page_get, json!({"site_id":site_id,"page":page_id,"details":{"wikitext":true,"compiled":true}})).unwrap();
    let selected = run_endpoint!(
        runner,
        page_history_revision,
        request(site_id, page_id, "wikidot", 0, true)
    )
    .unwrap();
    assert_eq!((selected.id, selected.number), (8_200_000, 0));
    assert_eq!(selected.source, "first\n旧\nlast\n");
    assert_eq!(
        selected.representation.as_deref(),
        Some("display-decoded-not-byte-exact")
    );
    let html = selected.rendered_html.unwrap();
    assert!(html.contains("旧"));
    assert!(!html.contains("新"));
    assert!(!html.contains("Current source"));
    let after = run_endpoint!(runner, page_get, json!({"site_id":site_id,"page":page_id,"details":{"wikitext":true,"compiled":true}})).unwrap();
    assert_eq!(
        (
            before.revision_id,
            before.page_revision_count,
            before.wikitext,
            before.compiled_body_html
        ),
        (
            after.revision_id,
            after.page_revision_count,
            after.wikitext,
            after.compiled_body_html
        )
    );
    assert!(
        run_endpoint!(
            runner,
            page_history_revision,
            request(site_id, page_id, "wikidot", 3, false)
        )
        .is_none()
    );
}

#[tokio::test]
async fn local_revision_uses_exact_stored_source_and_rejects_hidden_text() {
    let runner = TestRunner::setup().await;
    let (site_id, page_id, current) = fixture(&runner).await;
    let original = page_revision::Entity::find_by_id(current)
        .one(runner.context().transaction())
        .await
        .unwrap()
        .unwrap();
    let hash =
        TextService::create(runner.context(), "Historical //italic// source".into())
            .await
            .unwrap();
    let mut revision = original.into_active_model();
    revision.revision_id = sea_orm::ActiveValue::NotSet;
    revision.revision_number = Set(1);
    revision.wikitext_hash = Set(hash.to_vec());
    let historical = revision
        .insert(runner.context().transaction())
        .await
        .unwrap();
    let selected = run_endpoint!(
        runner,
        page_history_revision,
        request(site_id, page_id, "local", 1, true)
    )
    .unwrap();
    assert_eq!(selected.id, historical.revision_id);
    assert_eq!(selected.source, "Historical //italic// source");
    assert!(selected.rendered_html.unwrap().contains("Historical"));
    assert_eq!(selected.representation, None);
    let mut hidden = historical.into_active_model();
    hidden.hidden = Set(vec!["wikitext".into()]);
    hidden.update(runner.context().transaction()).await.unwrap();
    let error = run_endpoint_err!(
        runner,
        page_history_revision,
        request(site_id, page_id, "local", 1, false)
    );
    assert_contains_error!(error, ErrorType::Permission);
}

#[tokio::test]
async fn compares_full_unicode_source_lines_and_validates_inputs() {
    let runner = TestRunner::setup().await;
    let (site_id, page_id, current) = fixture(&runner).await;
    imported(&runner, site_id, page_id, current).await;
    let compare = |origin: &str, from: i32, to: i32| {
        json!({
            "site_id":site_id,"page_id":page_id,"origin":origin,"from":from,"to":to
        })
    };
    let diff = run_endpoint!(runner, page_history_compare, compare("wikidot", 0, 1));
    let lines: Vec<_> = diff
        .lines
        .iter()
        .map(|line| (line.kind.as_str(), line.text.as_str()))
        .collect();
    assert_eq!(
        lines,
        vec![
            ("same", "first\n"),
            ("delete", "旧\n"),
            ("insert", "新\n"),
            ("same", "last\n")
        ]
    );
    assert_eq!(
        diff.representation.as_deref(),
        Some("display-decoded-not-byte-exact")
    );
    for (from, to) in [(-1, 1), (0, -1), (0, 0), (0, 3)] {
        let error =
            run_endpoint_err!(runner, page_history_compare, compare("wikidot", from, to));
        assert_contains_error!(error, ErrorType::DatabaseImport);
    }
}

#[tokio::test]
async fn current_page_view_is_required_before_reading_either_origin() {
    let runner = TestRunner::setup().await;
    let (site_id, page_id, current) = fixture(&runner).await;
    imported(&runner, site_id, page_id, current).await;
    role_permission::Entity::delete_many()
        .filter(role_permission::Column::SiteId.eq(site_id))
        .filter(role_permission::Column::ResourceType.eq(Resource::Page))
        .filter(role_permission::Column::Action.eq(Action::View))
        .exec(runner.context().transaction())
        .await
        .unwrap();
    for origin in ["wikidot", "local"] {
        let error = run_endpoint_err!(
            runner,
            page_history_revision,
            request(site_id, page_id, origin, 0, false)
        );
        assert_contains_error!(error, ErrorType::Permission);
        let error = run_endpoint_err!(
            runner,
            page_history_compare,
            json!({"site_id":site_id,"page_id":page_id,"origin":origin,"from":0,"to":1})
        );
        assert_contains_error!(error, ErrorType::Permission);
    }
}
