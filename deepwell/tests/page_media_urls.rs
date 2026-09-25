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

#[tokio::test]
async fn bare_media_on_category_pages_preserves_full_page_name() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    for (slug, filename) in [
        ("character:melancholy", "Melancholy_Outfits"),
        ("character:melancholy:outfits", "Portrait.png"),
    ] {
        let created = run_endpoint!(
            runner,
            page_import,
            json!({
                "site_id": site_id,
                "user_id": ADMIN_USER_ID,
                "slug": slug,
                "title": slug,
                "wikitext": format!("[[image {filename}]]"),
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
            json!({"site_id": site_id, "page": created.page_id, "details": {"compiled": true}})
        )
        .unwrap();
        let html = page.compiled_body_html.unwrap();
        assert!(
            html.contains(&format!("src=\"/-/file/{slug}/{filename}\"")),
            "{slug}: {html}"
        );
    }
}

#[tokio::test]
async fn image_box_in_live_character_template_uses_character_attachment() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    for (slug, source) in [
        ("image-box", "[[image {$image}]]"),
        (
            "character:_template",
            "[[include image-box | image=%%form_data{image}%%]]\n\n====\n\n[[form]]\nfields:\n  image:\n    type: text\n[[/form]]\n",
        ),
        ("character:melancholy", "image: Melancholy_Outfits\n"),
    ] {
        run_endpoint!(
            runner,
            page_import,
            json!({
                "site_id": site_id,
                "user_id": ADMIN_USER_ID,
                "slug": slug,
                "title": slug,
                "wikitext": source,
                "alt_title": null,
                "layout": "wikidot",
                "revision_comments": "Media fixture",
                "bypass_filter": true,
                "ip_address": "127.0.0.1"
            })
        );
    }
    let page = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": "character:melancholy", "details": {"compiled": true}})
    )
    .unwrap();
    let html = page.compiled_body_html.unwrap();
    assert!(
        html.contains("src=\"/-/file/character:melancholy/Melancholy_Outfits\""),
        "{html}"
    );
}
