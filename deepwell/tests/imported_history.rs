//! Imported source history stays separate from current editable page revisions.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::prelude::ErrorType;
use deepwell::models::{role, role_permission};
use deepwell::services::RequestContext;
use deepwell::types::{Action, Reference, Resource};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Value, json};

#[tokio::test]
async fn imported_history_preserves_current_page_and_reads_multiple_pages() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site":"test"}))
        .unwrap()
        .site
        .site_id;
    let page = run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id":site_id,"user_id":ADMIN_USER_ID,"slug":"history-import-fixture",
            "title":"Current page","wikitext":"Current exact source\n", "alt_title":null,
            "layout":"wikidot","revision_comments":"History fixture marker","bypass_filter":true,"ip_address":"127.0.0.1"
        })
    );
    let before = run_endpoint!(runner, page_get, json!({"site_id":site_id,"page":page.page_id,"details":{"wikitext":true,"compiled":true}})).unwrap();
    let revisions: Vec<_> = (0..3).map(|number|json!({
        "source_revision_id":800000+number,"source_revision_number":number,
        "source_author_id":42,"source_created_at":"2021-04-29T12:00:00Z",
        "source_comments":"Archived edit","source_flags":if number==1 {vec!["F"]} else {vec!["S"]},
        "source_title":null,"source_slug":null,"source_tags":null,
        "wikitext":if number==0 {"Old text"} else {"New text"},
        "raw_source_html":"<div class=\"page-source\">\nArchived display\n</div>",
        "acquired_at":"2026-09-23T00:00:00Z","representation":"display-decoded-not-byte-exact"
    })).collect();
    let input = json!({"site_id":site_id,"page_id":page.page_id,"source_page_id":1310927108,"expected_revision_id":before.revision_id,"revisions":revisions});
    let first = run_endpoint!(runner, import_wikidot_history, input.clone());
    assert_eq!(first.inserted, 3);
    assert_eq!(
        run_endpoint!(runner, import_wikidot_history, input).inserted,
        0
    );
    let after=run_endpoint!(runner,page_get,json!({"site_id":site_id,"page":page.page_id,"details":{"wikitext":true,"compiled":true}})).unwrap();
    assert_eq!(before.revision_id, after.revision_id);
    assert_eq!(before.page_revision_count, after.page_revision_count);
    assert_eq!(before.wikitext, after.wikitext);
    assert_eq!(before.compiled_body_html, after.compiled_body_html);
    let first_page = run_endpoint!(
        runner,
        page_imported_history,
        json!({"site_id":site_id,"page_id":page.page_id,"before_revision":null,"limit":2})
    );
    assert_eq!(first_page.len(), 2);
    assert_eq!(first_page[0].source_revision_number, 2);
    assert_eq!(first_page[1].source_revision_number, 1);
    assert_eq!(first_page[1].source_flags, vec!["F"]);
    assert_eq!(first_page[1].source_tags, None);
    let second_page = run_endpoint!(
        runner,
        page_imported_history,
        json!({"site_id":site_id,"page_id":page.page_id,"before_revision":1,"limit":2})
    );
    assert_eq!(second_page.len(), 1);
    assert_eq!(second_page[0].source_revision_number, 0);
    let source = run_endpoint!(
        runner,
        page_imported_revision,
        json!({"site_id":site_id,"page_id":page.page_id,"source_revision_number":0})
    )
    .unwrap();
    assert_eq!(source.wikitext, "Old text");
    assert_eq!(source.metadata.source_author_id, Some(42));
}

async fn fixture_page(runner: &TestRunner, slug: &str) -> (i64, i64, i64) {
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let created = run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id": site_id,
            "user_id": ADMIN_USER_ID,
            "slug": slug,
            "title": "Current archived title",
            "wikitext": "Current archived source\n",
            "alt_title": null,
            "layout": "wikidot",
            "revision_comments": "Technical import marker",
            "bypass_filter": true,
            "ip_address": "127.0.0.1"
        })
    );
    let page = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": created.page_id, "details": {"wikitext": true}})
    )
    .unwrap();
    (site_id, page.page_id, page.revision_id)
}

fn archived_revision(number: i64) -> Value {
    json!({
        "source_revision_id": 900_100 + number,
        "source_revision_number": number,
        "source_author_id": 42,
        "source_created_at": "2021-04-29T12:00:00Z",
        "source_comments": "Source comment",
        "source_flags": ["S"],
        "source_title": null,
        "source_slug": null,
        "source_tags": null,
        "wikitext": format!("Historical body {number}"),
        "raw_source_html": format!("<div class=\"page-source\">\nHistorical body {number}\n</div>"),
        "acquired_at": "2026-09-23T00:00:00Z",
        "representation": "display-decoded-not-byte-exact"
    })
}

fn history_input(
    site_id: i64,
    page_id: i64,
    revision_id: i64,
    revisions: Vec<Value>,
) -> Value {
    json!({
        "site_id": site_id,
        "page_id": page_id,
        "source_page_id": 1_310_927_108,
        "expected_revision_id": revision_id,
        "revisions": revisions
    })
}

#[tokio::test]
async fn conflicting_and_duplicate_source_revisions_do_not_replace_stored_history() {
    let runner = TestRunner::setup().await;
    let (site_id, page_id, current_revision) =
        fixture_page(&runner, "history-conflict-fixture").await;
    let original = archived_revision(0);
    let input = history_input(site_id, page_id, current_revision, vec![original.clone()]);
    assert_eq!(
        run_endpoint!(runner, import_wikidot_history, input.clone()).inserted,
        1
    );

    let mut changed = original.clone();
    changed["wikitext"] = json!("Conflicting replacement text");
    let conflict = run_endpoint_err!(
        runner,
        import_wikidot_history,
        history_input(site_id, page_id, current_revision, vec![changed])
    );
    assert_contains_error!(conflict, ErrorType::DatabaseImport);
    let source = run_endpoint!(
        runner,
        page_imported_revision,
        json!({"site_id": site_id, "page_id": page_id, "source_revision_number": 0})
    )
    .unwrap();
    assert_eq!(source.wikitext, "Historical body 0");
    assert_eq!(
        run_endpoint!(runner, import_wikidot_history, input).inserted,
        0
    );

    let duplicate_id = vec![archived_revision(1), {
        let mut revision = archived_revision(2);
        revision["source_revision_id"] = json!(900_101);
        revision
    }];
    let error = run_endpoint_err!(
        runner,
        import_wikidot_history,
        history_input(site_id, page_id, current_revision, duplicate_id)
    );
    assert_contains_error!(error, ErrorType::DatabaseImport);
    let duplicate_number = vec![archived_revision(1), {
        let mut revision = archived_revision(1);
        revision["source_revision_id"] = json!(900_102);
        revision
    }];
    let error = run_endpoint_err!(
        runner,
        import_wikidot_history,
        history_input(site_id, page_id, current_revision, duplicate_number)
    );
    assert_contains_error!(error, ErrorType::DatabaseImport);
    let remaining = run_endpoint!(
        runner,
        page_imported_history,
        json!({"site_id": site_id, "page_id": page_id, "before_revision": null, "limit": 10})
    );
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].source_revision_id, 900_100);
}

#[tokio::test]
async fn history_import_rejects_a_page_from_a_different_site() {
    let runner = TestRunner::setup().await;
    let (site_id, page_id, current_revision) =
        fixture_page(&runner, "history-other-site-fixture").await;
    let another_site = run_endpoint!(runner, site_get, json!({"site": "scp-wiki"}))
        .unwrap()
        .site
        .site_id;
    assert_ne!(another_site, site_id);
    let error = run_endpoint_err!(
        runner,
        import_wikidot_history,
        history_input(
            another_site,
            page_id,
            current_revision,
            vec![archived_revision(0)]
        )
    );
    assert_contains_error!(error, ErrorType::DatabaseImport);
    assert!(run_endpoint!(
        runner,
        page_imported_history,
        json!({"site_id": site_id, "page_id": page_id, "before_revision": null, "limit": 10})
    ).is_empty());
}

#[tokio::test]
async fn intervening_page_edit_rejects_history_and_preserves_the_new_current_source() {
    let mut runner = TestRunner::setup().await;
    let slug = "history-intervening-edit-fixture";
    let (site_id, page_id, old_revision) = fixture_page(&runner, slug).await;
    runner.set_request_context(RequestContext {
        session: None,
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Id(page_id)),
    });
    let edited = run_endpoint!(
        runner,
        page_edit,
        json!({
            "site_id": site_id,
            "page": page_id,
            "last_revision_id": old_revision,
            "revision_comments": "New user edit",
            "user_id": ADMIN_USER_ID,
            "wikitext": "User edit after source acquisition\n",
            "ip_address": "127.0.0.1"
        })
    )
    .expect("page edit must create a later native revision");
    assert_ne!(edited.revision_id, old_revision);
    let error = run_endpoint_err!(
        runner,
        import_wikidot_history,
        history_input(site_id, page_id, old_revision, vec![archived_revision(0)])
    );
    assert_contains_error!(error, ErrorType::DatabaseImport);
    let current = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": page_id, "details": {"wikitext": true}
        })
    )
    .unwrap();
    assert_eq!(current.revision_id, edited.revision_id);
    assert_eq!(
        current.wikitext.as_deref(),
        Some("User edit after source acquisition\n")
    );
    assert!(run_endpoint!(runner, page_imported_history, json!({
        "site_id": site_id, "page_id": page_id, "before_revision": null, "limit": 10
    })).is_empty());
}

#[tokio::test]
async fn history_list_and_source_refuse_anonymous_page_view_denial() {
    let runner = TestRunner::setup().await;
    let (site_id, page_id, revision_id) =
        fixture_page(&runner, "history-private-fixture").await;
    let input = history_input(site_id, page_id, revision_id, vec![archived_revision(0)]);
    assert_eq!(
        run_endpoint!(runner, import_wikidot_history, input).inserted,
        1
    );
    let before = run_endpoint!(
        runner,
        page_imported_history,
        json!({
            "site_id": site_id, "page_id": page_id, "before_revision": null, "limit": 10
        })
    );
    assert_eq!(before.len(), 1);

    role_permission::Entity::delete_many()
        .filter(role_permission::Column::SiteId.eq(site_id))
        .filter(role_permission::Column::ResourceType.eq(Resource::Page))
        .filter(role_permission::Column::Action.eq(Action::View))
        .exec(runner.context().transaction())
        .await
        .expect("remove all anonymous page-view permissions in rollback fixture");
    let list_error = run_endpoint_err!(
        runner,
        page_imported_history,
        json!({
            "site_id": site_id, "page_id": page_id, "before_revision": null, "limit": 10
        })
    );
    assert_contains_error!(list_error, ErrorType::Permission);
    let source_error = run_endpoint_err!(
        runner,
        page_imported_revision,
        json!({
            "site_id": site_id, "page_id": page_id, "source_revision_number": 0
        })
    );
    assert_contains_error!(source_error, ErrorType::Permission);
}

#[tokio::test]
async fn history_reads_use_request_identity_not_json_user_id() {
    let mut runner = TestRunner::setup().await;
    let (site_id, page_id, revision_id) =
        fixture_page(&runner, "history-identity-fixture").await;
    run_endpoint!(
        runner,
        import_wikidot_history,
        history_input(site_id, page_id, revision_id, vec![archived_revision(0)])
    );
    let roles = role::Entity::find()
        .filter(role::Column::SiteId.eq(site_id))
        .filter(role::Column::Name.is_in(["anonymous", "everyone", "guest"]))
        .all(runner.context().transaction())
        .await
        .unwrap();
    assert!(!roles.is_empty());
    role_permission::Entity::delete_many()
        .filter(
            role_permission::Column::RoleId
                .is_in(roles.into_iter().map(|role| role.role_id)),
        )
        .filter(role_permission::Column::ResourceType.eq(Resource::Page))
        .filter(role_permission::Column::Action.eq(Action::View))
        .exec(runner.context().transaction())
        .await
        .unwrap();
    let request = json!({"site_id":site_id,"page_id":page_id,"before_revision":null,"limit":10,"user_id":ADMIN_USER_ID});
    let error = run_endpoint_err!(runner, page_imported_history, request);
    assert_contains_error!(error, ErrorType::Permission);
    let error = run_endpoint_err!(
        runner,
        page_imported_revision,
        json!({"site_id":site_id,"page_id":page_id,"source_revision_number":0,"user_id":ADMIN_USER_ID})
    );
    assert_contains_error!(error, ErrorType::Permission);
    runner.set_request_context(RequestContext {
        session: None,
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Id(page_id)),
    });
    assert_eq!(
        run_endpoint!(
            runner,
            page_imported_history,
            json!({"site_id":site_id,"page_id":page_id,"before_revision":null,"limit":10})
        )
        .len(),
        1
    );
    assert!(
        run_endpoint!(
            runner,
            page_imported_revision,
            json!({"site_id":site_id,"page_id":page_id,"source_revision_number":0})
        )
        .is_some()
    );
}
