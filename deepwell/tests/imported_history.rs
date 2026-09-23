//! Imported source history stays separate from current editable page revisions.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use serde_json::json;

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
