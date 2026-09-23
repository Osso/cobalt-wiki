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
            tags: vec![],
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

#[tokio::test]
async fn rerender_resolves_later_imported_target_without_new_revision() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let source = "[[[later-title-target|]]]";
    create_page(&runner, site_id, "earlier-title-source", "Source", source).await;
    let before = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": "earlier-title-source",
            "details": {"wikitext": true, "compiled": true}
        })
    )
    .unwrap();
    assert!(
        before
            .compiled_body_html
            .as_deref()
            .unwrap()
            .contains(">later-title-target</a>")
    );
    create_page(
        &runner,
        site_id,
        "later-title-target",
        "Resolved target",
        "Target",
    )
    .await;
    run_endpoint!(
        runner,
        page_rerender,
        json!({
            "site_id": site_id, "category_id": before.page_category_id, "page_id": before.page_id
        })
    );
    let after = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": before.page_id,
            "details": {"wikitext": true, "compiled": true}
        })
    )
    .unwrap();
    assert!(
        after
            .compiled_body_html
            .as_deref()
            .unwrap()
            .contains(">Resolved target</a>")
    );
    assert_eq!(after.revision_id, before.revision_id);
    assert_eq!(after.page_revision_count, before.page_revision_count);
    assert_eq!(after.revision_created_at, before.revision_created_at);
    assert_eq!(after.revision_user_id, before.revision_user_id);
    assert_eq!(after.wikitext.as_deref(), Some(source));
}
