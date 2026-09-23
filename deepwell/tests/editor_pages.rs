//! Editor autocomplete resolves only visible current pages in the trusted site.
#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::{ADMIN_USER_ID, SYSTEM_USER_ID};
use deepwell::error::ErrorType;
use deepwell::license::License;
use deepwell::models::{page, page_revision};
use deepwell::services::RequestContext;
use deepwell::services::category::CategoryService;
use deepwell::services::permission::PermissionService;
use deepwell::services::role::{
    InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::site::{CreateSite, SiteService};
use deepwell::types::{Action, Permission, Reference, Resource};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::{Value, json};

async fn setup() -> (TestRunner, i64) {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    (runner, site_id)
}

async fn import_page(
    runner: &TestRunner,
    site_id: i64,
    slug: &str,
    title: &str,
    body: &str,
) -> i64 {
    run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id": site_id, "user_id": ADMIN_USER_ID, "slug": slug,
            "title": title, "wikitext": body, "alt_title": null,
            "layout": "wikidot", "revision_comments": "Fixture", "bypass_filter": true,
            "ip_address": common::IP_ADDRESS
        })
    )
    .page_id
}

fn actor(runner: &mut TestRunner, site_id: i64, user_id: Option<i64>) {
    runner.set_request_context(RequestContext {
        site_id: Some(site_id),
        user_id,
        ..Default::default()
    });
}

async fn suggestions(runner: &TestRunner, query: &str) -> Value {
    serde_json::to_value(run_endpoint!(runner, editor_pages, json!({"query": query})))
        .unwrap()
}

async fn make_private_category(runner: &TestRunner, site_id: i64) {
    let category_id =
        CategoryService::get_or_create(runner.context(), site_id, "lookup-private")
            .await
            .unwrap()
            .category_id;
    let role_id = RoleService::create(
        runner.context(),
        InternalCreateRoleInput {
            site_id,
            name: "Private lookup reader".into(),
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

#[tokio::test]
async fn matches_slug_prefix_only_with_space_conversion_case_and_slug_order() {
    let (mut runner, site_id) = setup().await;
    import_page(
        &runner,
        site_id,
        "lookup:zebra",
        "Other",
        "lookup alpha in body",
    )
    .await;
    import_page(
        &runner,
        site_id,
        "lookup:elsewhere",
        "lookup alpha title",
        "Other",
    )
    .await;
    import_page(&runner, site_id, "lookup:alpha-z", "Z title", "Other").await;
    import_page(&runner, site_id, "lookup:alpha-a", "A title", "Other").await;
    actor(&mut runner, site_id, Some(ADMIN_USER_ID));
    assert_eq!(
        suggestions(&runner, "LOOKUP:ALPHA ").await,
        json!([
            {"slug": "lookup:alpha-a", "title": "A title"},
            {"slug": "lookup:alpha-z", "title": "Z title"}
        ])
    );
    assert_eq!(suggestions(&runner, "alpha").await, json!([]));
}

#[tokio::test]
async fn treats_like_metacharacters_as_literal_prefix_characters() {
    let (mut runner, site_id) = setup().await;
    for (index, slug) in ["literal%a", "literal_a", "literal\\a", "literal-x"]
        .into_iter()
        .enumerate()
    {
        let id =
            import_page(&runner, site_id, &format!("fixture-{index}"), slug, "Other")
                .await;
        page::ActiveModel {
            page_id: Set(id),
            slug: Set(slug.into()),
            ..Default::default()
        }
        .update(runner.context().transaction())
        .await
        .unwrap();
    }
    actor(&mut runner, site_id, Some(ADMIN_USER_ID));
    for (query, slug) in [
        ("literal%", "literal%a"),
        ("literal_", "literal_a"),
        ("literal\\", "literal\\a"),
    ] {
        assert_eq!(
            suggestions(&runner, query).await,
            json!([{"slug": slug, "title": slug}])
        );
    }
}

#[tokio::test]
async fn returns_only_current_revision_titles_and_live_pages() {
    let (mut runner, site_id) = setup().await;
    let current =
        import_page(&runner, site_id, "match-current", "Old title", "Other").await;
    let deleted =
        import_page(&runner, site_id, "match-deleted", "Deleted title", "Other").await;
    let missing =
        import_page(&runner, site_id, "match-missing", "Missing title", "Other").await;
    let original = deepwell::services::PageRevisionService::get_latest(
        runner.context(),
        site_id,
        current,
    )
    .await
    .unwrap();
    page_revision::ActiveModel {
        revision_id: Set(original.revision_id),
        title: Set("Current title".into()),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
    page::ActiveModel {
        page_id: Set(deleted),
        deleted_at: Set(Some(deepwell::utils::now())),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
    page::ActiveModel {
        page_id: Set(missing),
        latest_revision_id: Set(None),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
    actor(&mut runner, site_id, Some(ADMIN_USER_ID));
    assert_eq!(
        suggestions(&runner, "match-").await,
        json!([
            {"slug": "match-current", "title": "Current title"}
        ])
    );
}

#[tokio::test]
async fn trusted_site_and_anonymous_view_filter_apply_before_visible_limit() {
    let (mut runner, site_id) = setup().await;
    make_private_category(&runner, site_id).await;
    let public_id =
        CategoryService::get_or_create(runner.context(), site_id, "lookup-public")
            .await
            .unwrap()
            .category_id;
    let anonymous_role = RoleService::get(
        runner.context(),
        site_id,
        Reference::Slug("anonymous".into()),
    )
    .await
    .unwrap();
    PermissionService::update_permissions_for_role(
        runner.context(),
        UpdateRolePermissionsInput {
            site_id,
            role_reference: Reference::Id(anonymous_role.role_id),
            new_permissions: vec![Permission {
                resource_type: Resource::Page,
                resource_category: Some(Reference::Id(public_id)),
                action: Action::View,
            }],
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    for index in 0..25 {
        import_page(
            &runner,
            site_id,
            &format!("lookup-private:item-{index:02}"),
            "Secret",
            "Other",
        )
        .await;
    }
    for index in 0..22 {
        import_page(
            &runner,
            site_id,
            &format!("lookup-public:item-{index:02}"),
            &format!("Public {index}"),
            "Other",
        )
        .await;
    }
    let foreign_site = SiteService::create(
        runner.context(),
        CreateSite {
            slug: format!("lookup-foreign-{}", uuid::Uuid::new_v4().simple()),
            name: "Foreign lookup site".into(),
            tagline: String::new(),
            description: "Foreign lookup fixture".into(),
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
    import_page(
        &runner,
        foreign_site,
        "lookup-public:foreign",
        "Foreign secret",
        "Other",
    )
    .await;
    actor(&mut runner, site_id, None);
    let first_page = deepwell::services::PageService::get(
        runner.context(),
        site_id,
        Reference::Slug("lookup-public:item-00".into()),
    )
    .await
    .unwrap();
    let allowed = PermissionService::check_user_can(
        runner.context(),
        &deepwell::services::permission::CheckPermissionContext {
            user_id: None,
            site_id,
            page_reference: Some(Reference::Id(first_page.page_id)),
        },
        Permission {
            resource_type: Resource::Page,
            resource_category: Some(Reference::Id(first_page.page_category_id)),
            action: Action::View,
        },
    )
    .await
    .unwrap();
    assert!(
        allowed,
        "anonymous public category should be visible; category {:?} vs {:?}",
        first_page.page_category_id, public_id
    );
    let result = serde_json::to_value(run_endpoint!(
        runner,
        editor_pages,
        json!({
            "query": "lookup", "site_id": foreign_site, "user_id": ADMIN_USER_ID
        })
    ))
    .unwrap();
    assert_eq!(result.as_array().unwrap().len(), 20);
    assert_eq!(
        result[0],
        json!({"slug": "lookup-public:item-00", "title": "Public 0"})
    );
    assert_eq!(
        result[19],
        json!({"slug": "lookup-public:item-19", "title": "Public 19"})
    );
    assert!(!result.to_string().contains("Foreign secret"));
    assert!(!result.to_string().contains("Secret"));
}

#[tokio::test]
async fn rejects_queries_outside_two_to_two_hundred_characters() {
    let (mut runner, site_id) = setup().await;
    actor(&mut runner, site_id, Some(ADMIN_USER_ID));
    for query in ["", "a", "  "] {
        let error = run_endpoint_err!(runner, editor_pages, json!({"query": query}));
        assert_contains_error!(error, ErrorType::BadRequest);
    }
    let long_query = "a".repeat(201);
    let error = run_endpoint_err!(runner, editor_pages, json!({"query": long_query}));
    assert_contains_error!(error, ErrorType::BadRequest);
}
