//! Automatic link labels must come from the referenced site's real page revision.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::PageService;
use deepwell::services::page::CreatePage;
use serde_json::json;

async fn create_page(
    runner: &TestRunner,
    site_id: i64,
    slug: &str,
    title: &str,
    source: &str,
) {
    PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            slug: slug.into(),
            title: title.into(),
            wikitext: source.into(),
            alt_title: None,
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Link title fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("Fixture page creation failed");
}

#[tokio::test]
async fn automatic_titles_use_target_revision_and_preserve_explicit_labels() {
    let runner = TestRunner::setup().await;
    let local = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let foreign = run_endpoint!(runner, site_get, json!({"site": "scp-wiki"}))
        .unwrap()
        .site
        .site_id;
    let target = "link-title-fixture";
    create_page(
        &runner,
        local,
        target,
        "Research & <Guide>",
        "Local fixture",
    )
    .await;
    create_page(&runner, foreign, target, "Foreign title", "Foreign fixture").await;
    let source = "[[[link-title-fixture|]]]\n\n* **[[[link-title-fixture|]]]**\n\n[[[link-title-fixture|Explicit label]]]\n\n[[[:scp-wiki:link-title-fixture|]]]\n\n[[[link-title-missing|]]]";
    create_page(&runner, local, "link-title-source", "Source", source).await;
    let page = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": local,
            "page": "link-title-source",
            "details": {"wikitext": true, "compiled": true}
        })
    )
    .unwrap();
    assert_eq!(page.wikitext.as_deref(), Some(source));
    let html = page.compiled_body_html.unwrap();
    assert_eq!(
        html.matches(">Research &amp; &lt;Guide&gt;</a>").count(),
        2,
        "{html}"
    );
    assert!(html.contains(">Explicit label</a>"), "{html}");
    assert!(html.contains(">Foreign title</a>"), "{html}");
    assert!(html.contains(">link-title-missing</a>"), "{html}");
    assert!(!html.contains("TODO:"), "{html}");
}
