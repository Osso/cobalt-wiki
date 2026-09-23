#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::ErrorType;
use deepwell::services::RequestContext;
use deepwell::types::Reference;
use serde_json::json;

#[tokio::test]
async fn create_rejects_anonymous_and_forged_actor_or_site() {
    let mut runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .expect("seed site")
        .site
        .site_id;
    let payload = json!({
        "site_id": site_id, "slug": "forms-permission-test", "title": "Forged",
        "wikitext": "cannot persist", "revision_comments": "",
        "user_id": ADMIN_USER_ID, "ip_address": common::IP_ADDRESS
    });

    runner.set_request_context(RequestContext {
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug("forms-permission-test".into())),
        ..Default::default()
    });
    assert!(!run_endpoint!(runner, page_create_permission).can_create);
    let denied = run_endpoint_err!(runner, page_create, payload.clone());
    assert_contains_error!(denied, ErrorType::PermissionDenied);

    runner.set_request_context(RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id + 99999),
        page_reference: Some(Reference::Slug("forms-permission-test".into())),
        ..Default::default()
    });
    let denied = run_endpoint_err!(runner, page_create, payload.clone());
    assert_contains_error!(denied, ErrorType::PermissionDenied);

    runner.set_request_context(RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug("different-page".into())),
        ..Default::default()
    });
    let denied = run_endpoint_err!(runner, page_create, payload);
    assert_contains_error!(denied, ErrorType::PermissionDenied);

    let missing = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": "forms-permission-test"})
    );
    assert!(missing.is_none(), "denied requests must not insert a page");
}

#[tokio::test]
async fn create_with_trusted_context_roundtrips_content_and_tags() {
    let mut runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .expect("seed site")
        .site
        .site_id;
    runner.set_request_context(RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug("forms-roundtrip".into())),
        ..Default::default()
    });
    assert!(run_endpoint!(runner, page_create_permission).can_create);
    let created = run_endpoint!(
        runner,
        page_create,
        json!({
            "site_id": site_id, "slug": "forms-roundtrip", "title": "New page",
            "wikitext": "Visible body", "tags": ["alpha", "beta"],
            "revision_comments": "Created", "user_id": ADMIN_USER_ID,
            "ip_address": common::IP_ADDRESS
        })
    );
    assert_eq!(created.slug, "forms-roundtrip");
    let page = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": "forms-roundtrip", "details": {"wikitext": true}})
    )
    .expect("created page must be readable");
    assert_eq!(page.title, "New page");
    assert_eq!(page.wikitext.as_deref(), Some("Visible body"));
    assert_eq!(page.tags, ["alpha", "beta"]);
    assert_eq!(page.revision_user_id, ADMIN_USER_ID);
    assert_eq!(page.revision_id, created.revision_id);
}
