#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::SYSTEM_USER_ID;
use deepwell::error::ErrorType;
use deepwell::license::License;
use deepwell::models::user;
use deepwell::services::RequestContext;
use deepwell::services::category::CategoryService;
use deepwell::services::page::{CreatePage, PageService};
use deepwell::services::permission::PermissionService;
use deepwell::services::relation::{
    CreateSiteMember, RelationService, SiteMemberAccepted, SiteMemberData,
};
use deepwell::services::role::{
    InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::site::{CreateSite, SiteService};
use deepwell::services::user::{CreateUser, UserService};
use deepwell::types::{Action, Permission, Reference, Resource, UserType};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseBackend, Statement,
};
use serde_json::{Value, json};
use time::OffsetDateTime;

async fn site(runner: &TestRunner, slug: &str) -> i64 {
    let site_id = SiteService::create(
        runner.context(),
        CreateSite {
            slug: slug.into(),
            name: slug.into(),
            tagline: String::new(),
            description: "watching fixture".into(),
            default_page: None,
            layout: None,
            license: License::CcBySa40,
            locale: "en".into(),
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .site_id;
    let role_id = RoleService::create(
        runner.context(),
        InternalCreateRoleInput {
            site_id,
            name: "member".into(),
            description: None,
            is_virtual: true,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .role_id;
    PermissionService::update_permissions_for_role(
        runner.context(),
        UpdateRolePermissionsInput {
            site_id,
            role_reference: Reference::Id(role_id),
            new_permissions: vec![
                Permission {
                    resource_type: Resource::Site,
                    resource_category: None,
                    action: Action::View,
                },
                Permission {
                    resource_type: Resource::Page,
                    resource_category: None,
                    action: Action::View,
                },
            ],
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    site_id
}

async fn member(runner: &TestRunner, site_id: i64, name: &str) -> i64 {
    let user = UserService::create(
        runner.context(),
        CreateUser {
            user_type: UserType::Regular,
            name: name.into(),
            email: format!("{name}@example.com"),
            locales: vec!["en".into()],
            password: "test-password".into(),
            bypass_filter: true,
            bypass_email_verification: true,
            override_user_id: None,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    RelationService::create_site_member(
        runner.context(),
        CreateSiteMember {
            site_id,
            user_id: user.user_id,
            created_by: SYSTEM_USER_ID,
            metadata: SiteMemberData {
                accepted: SiteMemberAccepted::SelfJoined,
            },
        },
        common::IP_ADDRESS,
    )
    .await
    .unwrap();
    user.user_id
}

fn actor(runner: &mut TestRunner, site_id: i64, user_id: Option<i64>) {
    runner.set_request_context(RequestContext {
        user_id,
        site_id: Some(site_id),
        ..Default::default()
    });
}

async fn page(runner: &TestRunner, site_id: i64, slug: &str) -> (i64, i64) {
    let category = slug.split_once(':').unwrap().0;
    let category_id = CategoryService::get_or_create(runner.context(), site_id, category)
        .await
        .unwrap()
        .category_id;
    let page_id = PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            user_id: SYSTEM_USER_ID,
            slug: slug.into(),
            title: slug.into(),
            alt_title: None,
            wikitext: "Watchable fixture".into(),
            layout: None,
            revision_comments: "fixture".into(),
            tags: vec![],
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .page_id;
    (category_id, page_id)
}

async fn restrict_category(runner: &TestRunner, site_id: i64, category_id: i64) {
    let role_id = RoleService::create(
        runner.context(),
        InternalCreateRoleInput {
            site_id,
            name: "watch private reader".into(),
            description: None,
            is_virtual: true,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .role_id;
    PermissionService::update_permissions_for_role(
        runner.context(),
        UpdateRolePermissionsInput {
            site_id,
            role_reference: Reference::Id(role_id),
            new_permissions: vec![Permission {
                resource_type: Resource::Page,
                resource_category: Some(Reference::Id(category_id)),
                action: Action::View,
            }],
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
}

fn set_request(site_id: i64, scope: &str, target_id: i64, watching: bool) -> Value {
    json!({"site_id": site_id, "scope": scope, "target_id": target_id, "watching": watching})
}

async fn subscription_count(runner: &TestRunner, user_id: i64) -> i64 {
    runner
        .context()
        .transaction()
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT count(*) AS total FROM watch_subscription WHERE user_id = $1",
            [user_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "total")
        .unwrap()
}

#[tokio::test]
async fn signed_in_preferences_default_off_and_verified_email_required() {
    let mut runner = TestRunner::setup().await;
    let site_id = site(&runner, "watch-pref-site").await;
    let user_id = member(&runner, site_id, "WatchPrefMember").await;
    actor(&mut runner, site_id, None);
    assert_contains_error!(
        run_endpoint_err!(runner, watching_preferences_get, json!({})),
        ErrorType::Request
    );
    assert_contains_error!(
        run_endpoint_err!(
            runner,
            watching_preferences_set,
            json!({"email_enabled": false, "auto_watch": true})
        ),
        ErrorType::Request
    );

    actor(&mut runner, site_id, Some(user_id));
    assert_eq!(
        json!({"email_enabled": false, "auto_watch": false}),
        json!(run_endpoint!(runner, watching_preferences_get, json!({})))
    );
    assert_contains_error!(
        run_endpoint_err!(
            runner,
            watching_preferences_set,
            json!({"email_enabled": true, "auto_watch": true})
        ),
        ErrorType::BadRequest
    );
    assert_eq!(
        json!({"email_enabled": false, "auto_watch": true}),
        json!(run_endpoint!(
            runner,
            watching_preferences_set,
            json!({"email_enabled": false, "auto_watch": true, "user_id": SYSTEM_USER_ID})
        ))
    );

    user::ActiveModel {
        user_id: Set(user_id),
        email_verified_at: Set(Some(OffsetDateTime::now_utc())),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
    assert_eq!(
        json!({"email_enabled": true, "auto_watch": false}),
        json!(run_endpoint!(
            runner,
            watching_preferences_set,
            json!({"email_enabled": true, "auto_watch": false})
        ))
    );
    assert_eq!(
        json!({"email_enabled": true, "auto_watch": false}),
        json!(run_endpoint!(runner, watching_preferences_get, json!({})))
    );
    actor(&mut runner, site_id, None);
    assert_contains_error!(
        run_endpoint_err!(
            runner,
            watching_preferences_get,
            json!({"user_id": user_id})
        ),
        ErrorType::Request
    );
}

#[tokio::test]
async fn site_category_page_watches_dedupe_and_are_owned_by_actor() {
    let mut runner = TestRunner::setup().await;
    let site_id = site(&runner, "watch-scopes-site").await;
    let user_id = member(&runner, site_id, "WatchScopesMember").await;
    let other_id = member(&runner, site_id, "WatchScopesOther").await;
    let (category_id, page_id) = page(&runner, site_id, "public:watchable").await;
    actor(&mut runner, site_id, None);
    assert_contains_error!(
        run_endpoint_err!(runner, watching_subscriptions, json!({"site_id": site_id})),
        ErrorType::Request
    );
    assert_contains_error!(
        run_endpoint_err!(
            runner,
            watching_subscription_set,
            set_request(site_id, "page", page_id, true)
        ),
        ErrorType::Request
    );
    actor(&mut runner, site_id, Some(user_id));

    for (scope, target_id) in [
        ("site", site_id),
        ("category", category_id),
        ("page", page_id),
    ] {
        for _ in 0..2 {
            assert_eq!(
                json!({"watching": true}),
                json!(run_endpoint!(
                    runner,
                    watching_subscription_set,
                    set_request(site_id, scope, target_id, true)
                ))
            );
        }
    }
    assert_eq!(subscription_count(&runner, user_id).await, 3);
    let watches = json!(run_endpoint!(
        runner,
        watching_subscriptions,
        json!({"site_id": site_id})
    ));
    assert_eq!(
        watches,
        json!([
            {"scope": "site", "target_id": site_id},
            {"scope": "category", "target_id": category_id},
            {"scope": "page", "target_id": page_id},
        ])
    );
    actor(&mut runner, site_id, Some(other_id));
    assert_eq!(
        json!([]),
        json!(run_endpoint!(
            runner,
            watching_subscriptions,
            json!({"site_id": site_id})
        ))
    );
    assert_eq!(
        json!({"watching": false}),
        json!(run_endpoint!(
            runner,
            watching_subscription_set,
            set_request(site_id, "page", page_id, false)
        ))
    );
    assert_eq!(subscription_count(&runner, user_id).await, 3);
    actor(&mut runner, site_id, Some(user_id));
    for _ in 0..2 {
        assert_eq!(
            json!({"watching": false}),
            json!(run_endpoint!(
                runner,
                watching_subscription_set,
                set_request(site_id, "page", page_id, false)
            ))
        );
    }
    assert_eq!(subscription_count(&runner, user_id).await, 2);
}

#[tokio::test]
async fn cross_site_and_unreadable_targets_cannot_be_watched_but_can_be_unwatched() {
    let mut runner = TestRunner::setup().await;
    let site_id = site(&runner, "watch-privacy-site").await;
    let foreign_site = site(&runner, "watch-privacy-foreign").await;
    let user_id = member(&runner, site_id, "WatchPrivacyMember").await;
    let (public_category, public_page) = page(&runner, site_id, "public:ok").await;
    let (private_category, private_page) = page(&runner, site_id, "private:secret").await;
    let (foreign_category, foreign_page) =
        page(&runner, foreign_site, "public:foreign").await;
    actor(&mut runner, site_id, Some(user_id));
    run_endpoint!(
        runner,
        watching_subscription_set,
        set_request(site_id, "page", private_page, true)
    );
    run_endpoint!(
        runner,
        watching_subscription_set,
        set_request(site_id, "category", private_category, true)
    );
    restrict_category(&runner, site_id, private_category).await;

    for (scope, target_id) in [
        ("site", foreign_site),
        ("category", foreign_category),
        ("page", foreign_page),
        ("page", private_page),
        ("category", private_category),
    ] {
        let result = deepwell::endpoints::all::watching_subscription_set(
            runner.context(),
            common::make_params(set_request(site_id, scope, target_id, true)),
        )
        .await;
        match result {
            Err(error) => assert_contains_error!(error, ErrorType::PermissionDenied),
            Ok(_) => panic!("unexpected watch for {scope} {target_id}"),
        }
    }
    assert_eq!(
        json!({"watching": false}),
        json!(run_endpoint!(
            runner,
            watching_subscription_set,
            set_request(site_id, "page", private_page, false)
        ))
    );
    assert_eq!(
        json!({"watching": false}),
        json!(run_endpoint!(
            runner,
            watching_subscription_set,
            set_request(site_id, "category", private_category, false)
        ))
    );
    assert_eq!(
        json!([]),
        json!(run_endpoint!(
            runner,
            watching_subscriptions,
            json!({"site_id": foreign_site})
        ))
    );
    assert_eq!(
        json!([]),
        json!(run_endpoint!(
            runner,
            watching_subscriptions,
            json!({"site_id": site_id})
        ))
    );
    run_endpoint!(
        runner,
        watching_subscription_set,
        set_request(site_id, "page", public_page, true)
    );
    run_endpoint!(
        runner,
        watching_subscription_set,
        set_request(site_id, "category", public_category, true)
    );
    assert_eq!(subscription_count(&runner, user_id).await, 2);
}
