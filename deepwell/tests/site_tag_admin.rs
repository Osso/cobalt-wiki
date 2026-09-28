#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::prelude::*;
use deepwell::services::RequestContext;
use serde_json::json;

async fn site_id(runner: &TestRunner) -> i64 {
    run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id
}

fn act_as(runner: &mut TestRunner, site_id: i64, user_id: Option<i64>) {
    runner.set_request_context(RequestContext {
        site_id: Some(site_id),
        user_id,
        ..Default::default()
    });
}

async fn page(runner: &TestRunner, site_id: i64, slug: &str, tags: &[&str]) -> i64 {
    run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id": site_id, "user_id": ADMIN_USER_ID, "slug": slug,
            "title": slug, "wikitext": "Body", "alt_title": null,
            "layout": "wikidot", "revision_comments": "initial",
            "tags": tags, "bypass_filter": true, "ip_address": common::IP_ADDRESS
        })
    )
    .page_id
}

async fn tags(runner: &TestRunner, site_id: i64, page_id: i64) -> Vec<String> {
    run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id,"page": page_id})
    )
    .unwrap()
    .tags
}

#[tokio::test]
async fn admin_preview_merge_and_undo_preserve_other_tags() {
    let mut runner = TestRunner::setup().await;
    let site = site_id(&runner).await;
    let first = page(&runner, site, "tag-first", &["source", "other", "target"]).await;
    let second = page(&runner, site, "tag-second", &["source", "other"]).await;
    let unaffected = page(&runner, site, "tag-unaffected", &["other"]).await;
    act_as(&mut runner, site, Some(ADMIN_USER_ID));
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"source","target_tag":"target"})
    );
    assert_eq!(preview.count, 2);
    assert_eq!(
        preview.pages.iter().map(|p| p.page_id).collect::<Vec<_>>(),
        vec![first, second]
    );
    let applied = run_endpoint!(
        runner,
        site_tag_apply,
        json!({
            "source_tag":"source", "target_tag":"target", "preview_token":preview.preview_token,
            "ip_address":common::IP_ADDRESS
        })
    );
    assert_eq!(tags(&runner, site, first).await, ["target", "other"]);
    assert_eq!(tags(&runner, site, second).await, ["target", "other"]);
    assert_eq!(tags(&runner, site, unaffected).await, ["other"]);
    let history = run_endpoint!(runner, site_tag_history);
    assert_eq!(history[0].operation_id, applied.operation_id);
    assert!(!history[0].reversed);
    run_endpoint!(
        runner,
        site_tag_undo,
        json!({"operation_id":applied.operation_id,"ip_address":common::IP_ADDRESS})
    );
    assert_eq!(
        tags(&runner, site, first).await,
        ["source", "other", "target"]
    );
    assert_eq!(tags(&runner, site, second).await, ["source", "other"]);
    assert!(run_endpoint!(runner, site_tag_history)[0].reversed);
}

#[tokio::test]
async fn preview_staleness_and_later_edit_conflict_are_atomic() {
    let mut runner = TestRunner::setup().await;
    let site = site_id(&runner).await;
    let first = page(&runner, site, "tag-conflict-a", &["source"]).await;
    let second = page(&runner, site, "tag-conflict-b", &["source"]).await;
    act_as(&mut runner, site, Some(ADMIN_USER_ID));
    let old = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"source","target_tag":null})
    );
    let third = page(&runner, site, "tag-conflict-c", &["source"]).await;
    let err = run_endpoint_err!(
        runner,
        site_tag_apply,
        json!({"source_tag":"source","target_tag":null,"preview_token":old.preview_token,"ip_address":common::IP_ADDRESS})
    );
    assert_contains_error!(err, ErrorType::TagPreviewStale);
    assert_eq!(tags(&runner, site, first).await, ["source"]);
    let current = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"source","target_tag":null})
    );
    let applied = run_endpoint!(
        runner,
        site_tag_apply,
        json!({"source_tag":"source","target_tag":null,"preview_token":current.preview_token,"ip_address":common::IP_ADDRESS})
    );
    assert_eq!(tags(&runner, site, third).await, Vec::<String>::new());
    let current_revision = deepwell::services::PageService::get(
        runner.context(),
        site,
        deepwell::types::Reference::Id(second),
    )
    .await
    .unwrap()
    .latest_revision_id
    .unwrap();
    run_endpoint!(
        runner,
        page_edit,
        json!({"site_id":site,"page":second,"user_id":ADMIN_USER_ID,"last_revision_id":current_revision,"revision_comments":"later edit","tags":["other"],"preserve_draft":true,"ip_address":common::IP_ADDRESS})
    );
    let err = run_endpoint_err!(
        runner,
        site_tag_undo,
        json!({"operation_id":applied.operation_id,"ip_address":common::IP_ADDRESS})
    );
    assert_contains_error!(err, ErrorType::TagUndoConflict);
    assert_eq!(tags(&runner, site, first).await, Vec::<String>::new());
    assert_eq!(tags(&runner, site, second).await, ["other"]);
    assert!(!run_endpoint!(runner, site_tag_history)[0].reversed);
}

#[tokio::test]
async fn locked_page_prevents_partial_apply() {
    let mut runner = TestRunner::setup().await;
    let site = site_id(&runner).await;
    let first = page(&runner, site, "tag-locked-a", &["source"]).await;
    let second = page(&runner, site, "tag-locked-b", &["source"]).await;
    act_as(&mut runner, site, Some(ADMIN_USER_ID));
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"source","target_tag":"new"})
    );
    run_endpoint!(
        runner,
        page_lock_create,
        json!({
            "page": second, "lock_type": "permission-only", "expires_at": null,
            "reason": null, "override_existing": false, "ip_address": common::IP_ADDRESS
        })
    );
    let err = run_endpoint_err!(
        runner,
        site_tag_apply,
        json!({
            "source_tag":"source", "target_tag":"new", "preview_token":preview.preview_token,
            "ip_address":common::IP_ADDRESS
        })
    );
    assert_contains_error!(err, ErrorType::TagPageNotEditable);
    assert_eq!(tags(&runner, site, first).await, ["source"]);
    assert_eq!(tags(&runner, site, second).await, ["source"]);
    assert!(run_endpoint!(runner, site_tag_history).is_empty());
}

#[tokio::test]
async fn non_admin_cannot_preview_apply_read_history_or_undo() {
    let mut runner = TestRunner::setup().await;
    let site = site_id(&runner).await;
    page(&runner, site, "tag-private", &["source"]).await;
    act_as(&mut runner, site, None);
    assert_contains_error!(
        run_endpoint_err!(
            runner,
            site_tag_preview,
            json!({"source_tag":"source","target_tag":null})
        ),
        ErrorType::PermissionDenied
    );
    assert_contains_error!(
        run_endpoint_err!(runner, site_tag_history),
        ErrorType::PermissionDenied
    );
    assert_contains_error!(
        run_endpoint_err!(
            runner,
            site_tag_apply,
            json!({"source_tag":"source","target_tag":null,"preview_token":"a","ip_address":common::IP_ADDRESS})
        ),
        ErrorType::PermissionDenied
    );
    assert_contains_error!(
        run_endpoint_err!(
            runner,
            site_tag_undo,
            json!({"operation_id":1,"ip_address":common::IP_ADDRESS})
        ),
        ErrorType::PermissionDenied
    );
}
