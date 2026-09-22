//! Shared rendered pages expand readable local includes without changing source.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::PageService;
use deepwell::services::page::CreatePage;
use serde_json::json;

async fn import_page(runner: &TestRunner, site_id: i64, slug: &str, source: &str) {
    PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            slug: slug.into(),
            title: slug.into(),
            wikitext: source.into(),
            alt_title: None,
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Include rendering fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("import include fixture");
}

#[tokio::test]
async fn nested_local_includes_substitute_arguments_and_preserve_source() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    import_page(
        &runner,
        site_id,
        "include-fixture-inner",
        "Inner **{$word}**.",
    )
    .await;
    import_page(
        &runner,
        site_id,
        "include-fixture-outer",
        "Outer\n\n[[include include-fixture-inner | word={$word}]]",
    )
    .await;
    let source = "Before\n\n[[include include-fixture-outer | word=world]]\n\nAfter";
    import_page(&runner, site_id, "include-fixture-page", source).await;
    let page = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": "include-fixture-page", "details": {"wikitext": true, "compiled": true}})
    )
    .unwrap();
    assert_eq!(page.wikitext.as_deref(), Some(source));
    let html = page.compiled_body_html.unwrap();
    assert!(html.contains("Inner <strong>world</strong>"), "{html}");
    assert!(html.contains("Before") && html.contains("Outer") && html.contains("After"));
    assert!(!html.contains("[[include"), "{html}");
}
