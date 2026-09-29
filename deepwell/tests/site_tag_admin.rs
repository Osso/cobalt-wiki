#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::prelude::*;
use deepwell::models::session::Model as SessionModel;
use deepwell::services::import::{ImportPage, ImportPageRevision, ImportService};
use deepwell::services::page::{EditPage, EditPageBody};
use deepwell::services::{
    PageRevisionService, PageService, RequestContext, ServiceContext,
};
use deepwell::types::{Maybe, PageRevisionType, Reference};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};
use serde_json::json;
use std::time::Duration;
use time::macros::datetime;

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
    let first = page(
        &runner,
        site,
        "tag-first",
        &["source", "other", "other", "target"],
    )
    .await;
    let second = page(&runner, site, "tag-second", &["source", "other"]).await;
    let unaffected = page(&runner, site, "tag-unaffected", &["other"]).await;
    let initial_revision = PageService::get(runner.context(), site, Reference::Id(first))
        .await
        .unwrap()
        .latest_revision_id
        .unwrap();
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(first)),
        ..Default::default()
    });
    run_endpoint!(
        runner,
        page_draft_save,
        json!({
            "title":"Private draft", "wikitext":"Unpublished revision",
            "last_revision_id": initial_revision
        })
    );
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
    assert_eq!(
        tags(&runner, site, first).await,
        ["other", "other", "target"]
    );
    assert_eq!(tags(&runner, site, second).await, ["target", "other"]);
    assert_eq!(tags(&runner, site, unaffected).await, ["other"]);
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(first)),
        ..Default::default()
    });
    assert_eq!(
        run_endpoint!(runner, page_draft_get)
            .draft
            .unwrap()
            .wikitext,
        "Unpublished revision"
    );
    act_as(&mut runner, site, Some(ADMIN_USER_ID));
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
        ["source", "other", "other", "target"]
    );
    assert_eq!(tags(&runner, site, second).await, ["source", "other"]);
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(first)),
        ..Default::default()
    });
    assert_eq!(
        run_endpoint!(runner, page_draft_get)
            .draft
            .unwrap()
            .wikitext,
        "Unpublished revision"
    );
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
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(deepwell::types::Reference::Id(second)),
        ..Default::default()
    });
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
async fn locked_page_prevents_partial_undo() {
    let mut runner = TestRunner::setup().await;
    let site = site_id(&runner).await;
    let first = page(&runner, site, "tag-undo-locked-a", &["source"]).await;
    let second = page(&runner, site, "tag-undo-locked-b", &["source"]).await;
    act_as(&mut runner, site, Some(ADMIN_USER_ID));
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"source","target_tag":"new"})
    );
    let applied = run_endpoint!(
        runner,
        site_tag_apply,
        json!({
            "source_tag":"source", "target_tag":"new", "preview_token":preview.preview_token,
            "ip_address":common::IP_ADDRESS
        })
    );
    run_endpoint!(
        runner,
        page_lock_create,
        json!({
            "page":second, "lock_type":"permission-only", "expires_at":null,
            "reason":null, "override_existing":false, "ip_address":common::IP_ADDRESS
        })
    );
    let err = run_endpoint_err!(
        runner,
        site_tag_undo,
        json!({
            "operation_id":applied.operation_id, "ip_address":common::IP_ADDRESS
        })
    );
    assert_contains_error!(err, ErrorType::TagPageNotEditable);
    assert_eq!(tags(&runner, site, first).await, ["new"]);
    assert_eq!(tags(&runner, site, second).await, ["new"]);
    assert!(!run_endpoint!(runner, site_tag_history)[0].reversed);
}

#[tokio::test]
async fn importing_page_waits_for_inflight_site_tag_change() {
    let runner = TestRunner::setup().await;
    let site = site_id(&runner).await;
    let state = runner.state();
    let first = state.database.begin().await.unwrap();
    let second = state.database.begin().await.unwrap();
    let context = ServiceContext::new(state, &second);
    first
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT pg_advisory_xact_lock($1)",
            [site.into()],
        ))
        .await
        .unwrap();
    let mut import = Box::pin(ImportService::add_page(
        &context,
        ImportPage {
            page_id: -9282026,
            site_id: site,
            created_at: time::OffsetDateTime::now_utc(),
            slug: "tag-concurrent-import".to_owned(),
            locked: false,
            discussion_thread_id: None,
            ip_address: common::IP_ADDRESS,
        },
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(150), &mut import)
            .await
            .is_err(),
        "import raced tag lock"
    );
    first.rollback().await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(10), import)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.page_id, -9282026);
    second.rollback().await.unwrap();
}

#[tokio::test]
async fn importing_revision_waits_for_inflight_site_tag_change() {
    let runner = TestRunner::setup().await;
    let site = run_endpoint!(runner, site_get, json!({"site":"www"}))
        .unwrap()
        .site
        .site_id;
    let page = PageService::get(runner.context(), site, Reference::Slug("start".into()))
        .await
        .unwrap();
    let previous = PageRevisionService::get_latest(runner.context(), site, page.page_id)
        .await
        .unwrap();
    let state = runner.state();
    let first = state.database.begin().await.unwrap();
    let second = state.database.begin().await.unwrap();
    first
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT pg_advisory_xact_lock($1)",
            [site.into()],
        ))
        .await
        .unwrap();
    let context = ServiceContext::new(state, &second);
    let mut import = Box::pin(ImportService::add_page_revision(
        &context,
        ImportPageRevision {
            revision_id: -9282027,
            revision_type: PageRevisionType::Regular,
            created_at: time::OffsetDateTime::now_utc(),
            updated_at: None,
            revision_number: previous.revision_number + 1,
            page_id: page.page_id,
            site_id: site,
            user_id: ADMIN_USER_ID,
            wikitext: "Revision imported after site tag lock".to_owned(),
            comments: "concurrent imported revision".to_owned(),
            title: previous.title,
            slug: page.slug,
            tags: vec!["imported-tag".to_owned()],
        },
    ));
    assert!(
        tokio::time::timeout(Duration::from_secs(2), &mut import)
            .await
            .is_err(),
        "imported revision raced tag lock"
    );
    first.rollback().await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(10), import)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.page_revision_id, -9282027);
    second.rollback().await.unwrap();
}

#[tokio::test]
async fn ordinary_edit_waits_for_inflight_site_tag_change() {
    let runner = TestRunner::setup().await;
    let site = run_endpoint!(runner, site_get, json!({"site":"www"}))
        .unwrap()
        .site
        .site_id;
    let page = PageService::get(runner.context(), site, Reference::Slug("start".into()))
        .await
        .unwrap();
    let state = runner.state();
    let first = state.database.begin().await.unwrap();
    let second = state.database.begin().await.unwrap();
    first
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT pg_advisory_xact_lock($1)",
            [site.into()],
        ))
        .await
        .unwrap();
    let context = ServiceContext::new(state, &second).with_request(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(page.page_id)),
        ..Default::default()
    });
    let mut edit = Box::pin(PageService::edit(
        &context,
        EditPage {
            site_id: site,
            page: Reference::Id(page.page_id),
            last_revision_id: page.latest_revision_id.unwrap(),
            revision_comments: "concurrent tag edit".to_owned(),
            user_id: ADMIN_USER_ID,
            body: EditPageBody {
                tags: Maybe::Set(vec!["tag-concurrent".to_owned()]),
                preserve_draft: true,
                ..Default::default()
            },
            ip_address: common::IP_ADDRESS,
        },
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(150), &mut edit)
            .await
            .is_err(),
        "ordinary edit raced tag lock"
    );
    first.rollback().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(10), edit)
            .await
            .unwrap()
            .unwrap()
            .is_some()
    );
    second.rollback().await.unwrap();
}

#[tokio::test]
async fn non_admin_cannot_preview_apply_read_history_or_undo() {
    let mut runner = TestRunner::setup().await;
    let site = site_id(&runner).await;
    page(&runner, site, "tag-private", &["source"]).await;
    let member = run_endpoint!(
        runner,
        user_create,
        json!({
            "user_type":"regular", "name":"TagReader", "email":"tagreader@example.com",
            "locales":["en"], "password":"secret", "bypass_filter":true,
            "bypass_email_verification":true, "ip_address":common::IP_ADDRESS
        })
    )
    .user_id;
    run_endpoint!(
        runner,
        membership_set,
        json!({
            "site_id":site, "user_id":member,
            "metadata":{"accepted":{"cause":"accepted","user_id":ADMIN_USER_ID}},
            "created_by":ADMIN_USER_ID, "joined_at":"2021-04-29T14:55:00Z",
            "ip_address":common::IP_ADDRESS
        })
    );
    let member_role = run_endpoint!(runner, list_site_roles, json!({"site_id":site}))
        .into_iter()
        .find(|role| role.name == "member")
        .unwrap()
        .role_id;
    run_endpoint!(
        runner,
        grant_role_to_user,
        json!({
            "user_id":member,"role_id":member_role,"site_id":site,
            "assigning_user_id":ADMIN_USER_ID,"expires_at":null,
            "ip_address":common::IP_ADDRESS
        })
    );
    let callers = [
        RequestContext {
            site_id: Some(site),
            ..Default::default()
        },
        RequestContext {
            site_id: Some(site),
            user_id: Some(member),
            ..Default::default()
        },
        RequestContext {
            site_id: Some(site),
            user_id: Some(ADMIN_USER_ID),
            session: Some(SessionModel {
                session_token: "restricted-tag-test".into(),
                user_id: ADMIN_USER_ID,
                created_at: datetime!(2026-09-24 00:00 UTC),
                expires_at: datetime!(2099-01-01 00:00 UTC),
                ip_address: common::IP_ADDRESS.to_string(),
                user_agent: "test".into(),
                restricted: true,
            }),
            ..Default::default()
        },
    ];
    for caller in callers {
        runner.set_request_context(caller);
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
}
