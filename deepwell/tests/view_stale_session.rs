//! An unknown or expired session token views pages as an anonymous visitor,
//! as a Wikidot visitor whose session ended is simply logged out.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::services::view::GetPageViewOutput;
use serde_json::json;

#[tokio::test]
async fn stale_session_tokens_view_as_anonymous() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let params = |token: Option<&str>| {
        common::make_params(json!({
            "site_id": site_id, "session_token": token, "locales": ["en"],
            "route": {"slug": "start", "extra": ""},
        }))
    };

    let anonymous = deepwell::endpoints::view::page_view(runner.context(), params(None))
        .await
        .expect("anonymous view");
    let stale = deepwell::endpoints::view::page_view(
        runner.context(),
        params(Some("wj:stale-session-token")),
    )
    .await
    .expect("a stale token views as anonymous");
    match (anonymous, stale) {
        (
            GetPageViewOutput::Found {
                compiled_body_html: a,
                ..
            },
            GetPageViewOutput::Found {
                compiled_body_html: b,
                ..
            },
        ) => assert_eq!(a, b),
        (GetPageViewOutput::Missing { .. }, GetPageViewOutput::Missing { .. }) => {}
        other => panic!("stale token must match the anonymous view: {other:?}"),
    }

    let preload = deepwell::endpoints::view::preload_view(
        runner.context(),
        common::make_params(json!({
            "site_id": site_id, "session_token": "wj:stale-session-token", "locales": ["en"],
        })),
    )
    .await
    .expect("preload with a stale token");
    assert!(preload.viewer.user_session.is_none());
}
