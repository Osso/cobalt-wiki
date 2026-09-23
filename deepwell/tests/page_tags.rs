//! TagCloud and PagesByTag render the site's tags like Wikidot's system:page-tags.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::PageService;
use deepwell::services::page::CreatePage;
use deepwell::services::view::GetPageViewOutput;
use serde_json::json;

async fn import(
    runner: &TestRunner,
    site_id: i64,
    slug: &str,
    title: &str,
    source: &str,
    tags: &[&str],
) {
    PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            slug: slug.into(),
            title: title.into(),
            wikitext: source.into(),
            alt_title: None,
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Tag module fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("import fixture page");
}

async fn view(runner: &TestRunner, site_id: i64, extra: &str) -> String {
    let output = deepwell::endpoints::view::page_view(
        runner.context(),
        common::make_params(json!({
            "site_id": site_id, "session_token": null, "locales": ["en"],
            "route": {"slug": "tagcloud:page-tags", "extra": extra},
        })),
    )
    .await
    .expect("view tagcloud:page-tags");
    match output {
        GetPageViewOutput::Found {
            compiled_body_html, ..
        } => compiled_body_html,
        other => panic!("page must be found: {other:?}"),
    }
}

#[tokio::test]
async fn page_tags_show_the_cloud_and_the_pages_of_the_url_tag() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    import(
        &runner,
        site_id,
        "character:tagcloud-alli",
        "Alli",
        "Alli",
        &["tagcloud-alli", "_completed"],
    )
    .await;
    import(
        &runner,
        site_id,
        "writing:tagcloud-late",
        "(2021-05-01) Late",
        "Late",
        &["tagcloud-alli", "tagcloud-atley"],
    )
    .await;
    import(
        &runner,
        site_id,
        "writing:tagcloud-early",
        "(2020-01-01) Early",
        "Early",
        &["tagcloud-atley"],
    )
    .await;
    import(
        &runner,
        site_id,
        "tagcloud:page-tags",
        "Page Tags",
        "[[module TagCloud limit=\"200\" target=\"tagcloud:page-tags\" maxFontSize=\"225%\" minFontSize=\"100%\"]]\n\n[[module PagesByTag]]",
        &[],
    )
    .await;

    let stored = view(&runner, site_id, "").await;
    let alli = stored
        .find(r#"href="/tagcloud:page-tags/tag/tagcloud-alli""#)
        .expect(&stored);
    let atley = stored
        .find(r#"href="/tagcloud:page-tags/tag/tagcloud-atley""#)
        .expect(&stored);
    assert!(alli < atley, "tags are in name order: {stored}");
    assert!(
        !stored.contains(">_completed</a>"),
        "hidden tags stay out: {stored}"
    );
    assert!(!stored.contains("tagged-pages-list"), "{stored}");

    let atley_pages = view(&runner, site_id, "tag/tagcloud-atley").await;
    let list = &atley_pages[atley_pages.find("<h2>").expect(&atley_pages)..];
    assert_eq!(
        list,
        "<h2>List of pages tagged with <em>tagcloud-atley</em>:</h2>\
         <div class=\"pages-list\" id=\"tagged-pages-list\">\
         <div class=\"pages-list-item\"><div class=\"title\"><a href=\"/writing:tagcloud-early\">(2020-01-01) Early</a></div></div>\
         <div class=\"pages-list-item\"><div class=\"title\"><a href=\"/writing:tagcloud-late\">(2021-05-01) Late</a></div></div>\
         </div>",
    );

    let hidden = view(&runner, site_id, "tag/_completed").await;
    assert!(
        hidden.contains(r#"<a href="/character:tagcloud-alli">Alli</a>"#),
        "{hidden}"
    );
}
