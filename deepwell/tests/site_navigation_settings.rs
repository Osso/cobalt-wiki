#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::SYSTEM_USER_ID;
use serde_json::json;

#[tokio::test]
async fn site_update_persists_navigation_pages_and_preserves_omitted_fields() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({ "site": "test" }))
        .expect("Seeded test site not found")
        .site
        .site_id;

    run_endpoint!(
        runner,
        site_update,
        json!({
            "site": site_id,
            "user_id": SYSTEM_USER_ID,
            "top_bar_page": "custom-top",
            "side_bar_page": "",
            "ip_address": "127.0.0.1",
        }),
    );

    let site = run_endpoint!(runner, site_get, json!({ "site": site_id }))
        .expect("Updated site not found")
        .site;
    assert_eq!(site.top_bar_page, "custom-top");
    assert_eq!(site.side_bar_page, "");

    run_endpoint!(
        runner,
        site_update,
        json!({
            "site": site_id,
            "user_id": SYSTEM_USER_ID,
            "side_bar_page": "enabled-side",
            "ip_address": "127.0.0.1",
        }),
    );

    let site = run_endpoint!(runner, site_get, json!({ "site": site_id }))
        .expect("Updated site not found")
        .site;
    assert_eq!(site.top_bar_page, "custom-top");
    assert_eq!(site.side_bar_page, "enabled-side");
}
