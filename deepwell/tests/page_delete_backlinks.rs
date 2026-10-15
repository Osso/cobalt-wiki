//! Deleting a page whose backlinks include an already deleted page.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::RequestContext;
use deepwell::types::Reference;
use serde_json::json;

async fn import(runner: &TestRunner, site: i64, slug: &str, source: &str) -> i64 {
    run_endpoint!(runner, page_import, json!({
        "site_id":site, "user_id":ADMIN_USER_ID, "slug":slug, "title":slug,
        "wikitext":source, "alt_title":null, "layout":"wikidot", "tags":[],
        "bypass_filter":true, "revision_comments":"fixture", "ip_address":common::IP_ADDRESS
    }))
    .page_id
}

async fn delete(runner: &mut TestRunner, site: i64, page: i64) {
    let latest = run_endpoint!(
        runner,
        page_get,
        json!({"site_id":site, "page":page, "details":{}})
    )
    .unwrap();
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(page)),
        ..Default::default()
    });
    run_endpoint!(
        runner,
        page_delete,
        json!({"site_id":site, "page":page, "last_revision_id":latest.revision_id,
               "revision_comments":"delete", "user_id":ADMIN_USER_ID,
               "ip_address":common::IP_ADDRESS})
    );
}

#[tokio::test]
async fn page_linked_only_from_a_deleted_page_can_be_deleted() {
    let mut runner = TestRunner::setup_with_idle_job_workers().await;
    let site = run_endpoint!(runner, site_get, json!({"site":"test"}))
        .unwrap()
        .site
        .site_id;
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        ..Default::default()
    });
    let target = import(&runner, site, "link-target", "Target").await;
    let linker = import(&runner, site, "link-source", "See [[[link-target]]].").await;

    delete(&mut runner, site, linker).await;
    delete(&mut runner, site, target).await;

    let deleted = run_endpoint!(
        runner,
        page_get,
        json!({"site_id":site, "page":target, "details":{}})
    );
    assert!(deleted.is_none(), "target page is still live");
}
