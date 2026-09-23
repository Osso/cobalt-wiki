//! Pruning orphaned text must retain imported historical source bodies.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::{page_revision, text};
use deepwell::services::TextService;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, ExprTrait, QueryFilter, Set};
use serde_json::json;

#[tokio::test]
async fn pruning_removes_orphan_but_preserves_imported_history_body() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let page = run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id": site_id,
            "user_id": ADMIN_USER_ID,
            "slug": "history-prune-fixture",
            "title": "Current source",
            "wikitext": "Current page remains intact",
            "alt_title": null,
            "layout": "wikidot",
            "revision_comments": "Technical import marker",
            "bypass_filter": true,
            "ip_address": "127.0.0.1"
        })
    );
    let current = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": page.page_id, "details": {"wikitext": true}})
    )
    .unwrap();
    let historical_text = "Older source survives pruning";
    run_endpoint!(
        runner,
        import_wikidot_history,
        json!({
            "site_id": site_id,
            "page_id": page.page_id,
            "source_page_id": 1310927108,
            "expected_revision_id": current.revision_id,
            "revisions": [{
                "source_revision_id": 1364785612,
                "source_revision_number": 0,
                "source_author_id": 42,
                "source_created_at": "2021-04-29T12:00:00Z",
                "source_comments": "Original edit",
                "source_flags": ["N"],
                "source_title": null,
                "source_slug": null,
                "source_tags": null,
                "wikitext": historical_text,
                "raw_source_html": "<div class=\"page-source\">\nOlder source survives pruning\n</div>",
                "acquired_at": "2026-09-23T00:00:00Z",
                "representation": "display-decoded-not-byte-exact"
            }]
        })
    );
    let orphan_hash = TextService::create(runner.context(), "Unused orphan text".into())
        .await
        .unwrap();
    let historical_hash = TextService::create(runner.context(), historical_text.into())
        .await
        .unwrap();
    assert!(
        TextService::exists(runner.context(), &orphan_hash)
            .await
            .unwrap()
    );
    assert!(
        TextService::exists(runner.context(), &historical_hash)
            .await
            .unwrap()
    );

    // Existing seed revisions with absent nav regions make NOT IN match no rows.
    // Supply a valid already-referenced body hash in this rollback transaction
    // so the prune candidate path really executes.
    let revisions = page_revision::Entity::find()
        .filter(
            page_revision::Column::CompiledTopBarHtmlHash
                .is_null()
                .or(page_revision::Column::CompiledSideBarHtmlHash.is_null()),
        )
        .all(runner.context().transaction())
        .await
        .unwrap();
    for revision in revisions {
        let body_hash = revision.compiled_body_html_hash.clone();
        let mut model: page_revision::ActiveModel = revision.into();
        if model.compiled_top_bar_html_hash.as_ref() == &None {
            model.compiled_top_bar_html_hash = Set(Some(body_hash.clone()));
        }
        if model.compiled_side_bar_html_hash.as_ref() == &None {
            model.compiled_side_bar_html_hash = Set(Some(body_hash));
        }
        model.update(runner.context().transaction()).await.unwrap();
    }

    TextService::prune(runner.context())
        .await
        .expect("prune orphan text");
    assert_eq!(
        TextService::get_optional(runner.context(), &orphan_hash)
            .await
            .unwrap(),
        None
    );
    assert!(
        text::Entity::find_by_id(historical_hash.to_vec())
            .one(runner.context().transaction())
            .await
            .unwrap()
            .is_some()
    );
    let stored = run_endpoint!(
        runner,
        page_imported_revision,
        json!({"site_id": site_id, "page_id": page.page_id, "source_revision_number": 0})
    )
    .unwrap();
    assert_eq!(stored.wikitext, historical_text);
    let unchanged = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": page.page_id, "details": {"wikitext": true}})
    )
    .unwrap();
    assert_eq!(unchanged.wikitext, current.wikitext);
    assert_eq!(unchanged.revision_id, current.revision_id);
}
