//! Wikidot uses the first colon as category separator; later colons remain in the name.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use serde_json::json;

async fn import_page(
    runner: &TestRunner,
    site_id: i64,
    slug: &str,
    title: &str,
    source: &str,
) {
    run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id": site_id, "user_id": ADMIN_USER_ID,
            "slug": slug, "title": title, "wikitext": source,
            "alt_title": null, "layout": "wikidot",
            "revision_comments": "Canonical name fixture", "bypass_filter": true,
            "ip_address": "127.0.0.1"
        })
    );
}

#[tokio::test]
async fn multicolon_names_share_source_category_and_link_to_exact_target() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let canonical = "writing:2026-01-02:chapter-one";
    import_page(&runner, site_id, "writing:plain", "Plain", "Plain text").await;
    import_page(
        &runner,
        site_id,
        canonical,
        "Canonical target",
        "Exact source",
    )
    .await;
    import_page(
        &runner,
        site_id,
        "writing-2026-01-02:chapter-one",
        "Different page",
        "Different source",
    )
    .await;
    let plain = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": "writing:plain"})
    )
    .unwrap();
    let target = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": canonical})
    )
    .unwrap();
    assert_eq!(target.page_category_id, plain.page_category_id);
    assert_eq!(target.page_category_slug, "writing");
    let source = "[[[writing:2026-01-02:chapter-one|]]]";
    import_page(
        &runner,
        site_id,
        "canonical-link-source",
        "Link source",
        source,
    )
    .await;
    let link = run_endpoint!(runner, page_get, json!({
        "site_id": site_id, "page": "canonical-link-source", "details": {"wikitext": true, "compiled": true}
    })).unwrap();
    assert_eq!(link.wikitext.as_deref(), Some(source));
    let html = link.compiled_body_html.unwrap();
    assert!(
        html.contains("href=\"/writing:2026-01-02:chapter-one\""),
        "{html}"
    );
    assert!(html.contains(">Canonical target</a>"), "{html}");
    assert!(!html.contains("Different page"), "{html}");
}
