//! A Wikidot site member becomes a native account through the import RPCs:
//! wikidot_user record, site membership, role grant, then activation with the
//! site join time as the account creation time.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::prelude::*;
use serde_json::json;
use time::macros::datetime;

const USER_ID: i64 = 7444794;

#[tokio::test]
async fn wikidot_member_becomes_account_with_join_time_and_role() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;

    run_endpoint!(
        runner,
        import_wikidot_user,
        json!({
            "user_id": USER_ID,
            "created_at": "2020-11-03T08:00:00Z",
            "fetched_at": "2026-09-24T00:00:00Z",
            "user_type": "extant", "name": "OzmaAsimov", "slug": "ozmaasimov",
            "avatar_uploaded_blob_id": null,
            "real_name": null, "gender": null, "birthday": null, "location": null,
            "biography": null, "website": null,
            "karma": 3, "is_pro": false,
            "importing_user_id": ADMIN_USER_ID,
            "ip_address": common::IP_ADDRESS,
        }),
    );
    run_endpoint!(
        runner,
        membership_set,
        json!({
            "site_id": site_id, "user_id": USER_ID,
            "metadata": {"accepted": {"cause": "accepted", "user_id": ADMIN_USER_ID}},
            "created_by": ADMIN_USER_ID,
            "ip_address": common::IP_ADDRESS,
        }),
    );
    let admin = run_endpoint!(runner, list_site_roles, json!({"site_id": site_id}))
        .into_iter()
        .find(|role| role.name == "admin")
        .expect("test site has an admin role");
    run_endpoint!(
        runner,
        grant_role_to_user,
        json!({
            "user_id": USER_ID, "role_id": admin.role_id, "site_id": site_id,
            "assigning_user_id": ADMIN_USER_ID, "expires_at": null,
            "ip_address": common::IP_ADDRESS,
        }),
    );

    let user = run_endpoint!(
        runner,
        user_activate_from_wikidot,
        json!({
            "user_id": USER_ID, "user_type": "regular",
            "email": "wikidot-7444794@members.invalid", "locales": ["en"],
            "password": "secret nobody is told",
            "bypass_filter": true, "bypass_email_verification": true,
            "created_at": "2021-04-29T14:55:00Z",
            "ip_address": common::IP_ADDRESS,
        }),
    );
    assert_eq!(user.user_id, USER_ID);
    assert_eq!(user.name, "OzmaAsimov");
    assert_eq!(user.slug, "ozmaasimov");
    assert_eq!(user.created_at, datetime!(2021-04-29 14:55:00 UTC));

    let roles = run_endpoint!(
        runner,
        get_user_roles,
        json!({"site_id": site_id, "user_id": USER_ID}),
    );
    let names: Vec<_> = roles.iter().map(|role| role.name.as_str()).collect();
    assert!(names.contains(&"admin"), "roles: {names:?}");

    for password in ["", "ozmaasimov", "password"] {
        let error = run_endpoint_err!(
            runner,
            auth_login,
            json!({
                "name_or_email": "ozmaasimov", "password": password,
                "ip_address": common::IP_ADDRESS, "user_agent": "test",
            }),
        );
        assert_contains_error!(
            error,
            ErrorType::EmptyPassword | ErrorType::InvalidAuthentication,
        );
    }
}
