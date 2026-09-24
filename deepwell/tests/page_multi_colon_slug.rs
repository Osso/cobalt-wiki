//! Wikidot page names keep every colon after the category:
//! `writing:a:b` is category `writing`, page `a:b`. Such pages are created
//! and viewed at their own slug, without a redirect to `writing-a:b`.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::RequestContext;
use deepwell::services::view::GetPageViewOutput;
use deepwell::types::Reference;
use serde_json::json;

const SLUG: &str = "writing:2022-03-27-slave-pens:freedom-or-vengeance";

#[tokio::test]
async fn multi_colon_slug_is_created_and_viewed_without_redirect() {
    let mut runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    runner.set_request_context(RequestContext {
        session: None,
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug(SLUG.into())),
    });

    let created = run_endpoint!(
        runner,
        page_create,
        json!({
            "site_id": site_id,
            "wikitext": "Freedom or vengeance",
            "title": "Freedom or Vengeance",
            "alt_title": null,
            "slug": SLUG,
            "layout": null,
            "revision_comments": "",
            "user_id": ADMIN_USER_ID,
            "ip_address": common::IP_ADDRESS,
        }),
    );
    assert_eq!(created.slug, SLUG);

    let view = deepwell::endpoints::view::page_view(
        runner.context(),
        common::make_params(json!({
            "site_id": site_id, "session_token": null, "locales": ["en"],
            "route": {"slug": SLUG, "extra": ""},
        })),
    )
    .await
    .expect("page view");
    match view {
        GetPageViewOutput::Found {
            page,
            redirect_page,
            ..
        } => {
            assert_eq!(redirect_page, None);
            assert_eq!(page.slug, SLUG);
        }
        other => panic!("multi-colon page must be found: {other:?}"),
    }
}
