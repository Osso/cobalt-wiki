//! Viewing a page compiled by another renderer build rerenders it first.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::page_revision;
use deepwell::services::page::CreatePage;
use deepwell::services::render::COMPILED_GENERATOR;
use deepwell::services::view::GetPageViewOutput;
use deepwell::services::{PageRevisionService, PageService, TextService};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;

#[tokio::test]
async fn viewing_a_page_from_another_renderer_build_rerenders_it() {
    let runner = TestRunner::setup().await;
    let ctx = runner.context();
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let page_id = PageService::import(
        ctx,
        CreatePage {
            site_id,
            slug: "renderer-upgrade".into(),
            title: "Renderer upgrade".into(),
            wikitext: "Old body".into(),
            alt_title: None,
            tags: vec![],
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Lazy rerender fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .page_id;

    // Stored HTML from an older build; the renderer now produces "New body".
    let revision = PageRevisionService::get_latest(ctx, site_id, page_id)
        .await
        .unwrap();
    let source = TextService::create(ctx, "New body".into()).await.unwrap();
    page_revision::ActiveModel {
        revision_id: Set(revision.revision_id),
        wikitext_hash: Set(source.to_vec()),
        compiled_generator: Set("ftml v0.0.1 old-build".into()),
        ..Default::default()
    }
    .update(ctx.transaction())
    .await
    .unwrap();

    let view = deepwell::endpoints::view::page_view(
        ctx,
        common::make_params(json!({
            "site_id": site_id, "session_token": null, "locales": ["en"],
            "route": {"slug": "renderer-upgrade", "extra": ""},
        })),
    )
    .await
    .unwrap();
    match view {
        GetPageViewOutput::Found {
            compiled_body_html, ..
        } => {
            assert!(
                compiled_body_html.contains("New body"),
                "{compiled_body_html}"
            )
        }
        other => panic!("page must be found: {other:?}"),
    }
    let revision = PageRevisionService::get_latest(ctx, site_id, page_id)
        .await
        .unwrap();
    assert_eq!(revision.compiled_generator, *COMPILED_GENERATOR);
}
