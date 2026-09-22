//! Same-site media must use the deployed site's authenticated file route.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use serde_json::json;

#[tokio::test]
async fn local_media_uses_same_origin_and_preserves_other_origins() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let source = "[[image banner.png]]\n\n[[image images/logo.png]]\n\n[[image test/images/logo.png]]\n\n[[image https://cdn.example.org/photo.png]]\n\n[[image other/images/logo.png]]";
    let created = run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id": site_id,
            "user_id": ADMIN_USER_ID,
            "slug": "media-preview",
            "title": "Media preview",
            "wikitext": source,
            "alt_title": null,
            "layout": "wikidot",
            "revision_comments": "Media fixture",
            "bypass_filter": true,
            "ip_address": "127.0.0.1"
        })
    );
    let page = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id,
            "page": created.page_id,
            "details": {"wikitext": true, "compiled": true}
        })
    )
    .unwrap();
    assert_eq!(page.wikitext.as_deref(), Some(source));
    let html = page.compiled_body_html.unwrap();
    assert!(
        html.contains("src=\"/-/file/media-preview/banner.png\""),
        "{html}"
    );
    assert_eq!(
        html.matches("src=\"/-/file/images/logo.png\"").count(),
        2,
        "{html}"
    );
    assert!(
        html.contains("src=\"https://cdn.example.org/photo.png\""),
        "{html}"
    );
    assert!(
        html.contains("src=\"https://other.wjfiles.com/local--files/images/logo.png\""),
        "{html}"
    );
    assert!(!html.contains("test.wjfiles.com"), "{html}");
}
