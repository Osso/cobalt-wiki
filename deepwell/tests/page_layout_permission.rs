#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::{ADMIN_USER_ID, SAMPLE_USER_ID, SYSTEM_USER_ID};
use deepwell::error::ErrorType;
use deepwell::services::RequestContext;
use deepwell::services::page::{CreatePage, PageService};
use deepwell::services::permission::PermissionService;
use deepwell::services::role::{
    GrantUserRoleInput, InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::types::{Action, Permission, Reference, Resource};
use ftml::layout::Layout;
use serde_json::json;

async fn setup() -> (TestRunner, i64, i64, i64) {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let editable = create_page(&runner, site_id, "editable:layout").await;
    let protected = create_page(&runner, site_id, "protected:layout").await;
    let role = RoleService::create(
        runner.context(),
        InternalCreateRoleInput {
            site_id,
            name: "Layout editor".into(),
            description: None,
            is_virtual: false,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    let category_id =
        PageService::get(runner.context(), site_id, Reference::Id(editable))
            .await
            .unwrap()
            .page_category_id;
    PermissionService::update_permissions_for_role(
        runner.context(),
        UpdateRolePermissionsInput {
            site_id,
            role_reference: Reference::Id(role.role_id),
            new_permissions: vec![Permission {
                resource_type: Resource::Page,
                resource_category: Some(Reference::Id(category_id)),
                action: Action::Edit,
            }],
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    RoleService::grant_role_to_user(
        runner.context(),
        GrantUserRoleInput {
            site_id,
            user_id: SAMPLE_USER_ID,
            role_id: role.role_id,
            assigning_user_id: SYSTEM_USER_ID,
            expires_at: None,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    (runner, site_id, editable, protected)
}

async fn create_page(runner: &TestRunner, site_id: i64, slug: &str) -> i64 {
    PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            user_id: SYSTEM_USER_ID,
            slug: slug.into(),
            title: slug.into(),
            alt_title: None,
            wikitext: "Fixture".into(),
            layout: None,
            revision_comments: "Fixture".into(),
            tags: vec![],
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .page_id
}

fn target(runner: &mut TestRunner, site_id: i64, page_id: i64, actor: Option<i64>) {
    runner.set_request_context(RequestContext {
        user_id: actor,
        site_id: Some(site_id),
        page_reference: Some(Reference::Id(page_id)),
        ..Default::default()
    });
}

async fn layout(runner: &TestRunner, site_id: i64, page_id: i64) -> Option<Layout> {
    PageService::get(runner.context(), site_id, Reference::Id(page_id))
        .await
        .unwrap()
        .layout
        .as_deref()
        .map(|value| value.parse().unwrap())
}

fn input(
    site_id: i64,
    page_id: i64,
    user_id: i64,
    layout: Option<&str>,
) -> serde_json::Value {
    json!({
        "site_id": site_id,
        "page_id": page_id,
        "user_id": user_id,
        "ip_address": common::IP_ADDRESS,
        "layout": layout,
    })
}

#[tokio::test]
async fn denied_actor_cannot_change_layout() {
    let (mut runner, site_id, _, protected) = setup().await;
    target(&mut runner, site_id, protected, Some(SAMPLE_USER_ID));
    let error = run_endpoint_err!(
        runner,
        page_set_layout,
        input(site_id, protected, SAMPLE_USER_ID, Some("wikidot")),
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(layout(&runner, site_id, protected).await, None);
}

#[tokio::test]
async fn authorized_actor_can_set_and_clear_layout() {
    let (mut runner, site_id, editable, _) = setup().await;
    target(&mut runner, site_id, editable, Some(SAMPLE_USER_ID));
    run_endpoint!(
        runner,
        page_set_layout,
        input(site_id, editable, SAMPLE_USER_ID, Some("wikidot")),
    );
    assert_eq!(
        layout(&runner, site_id, editable).await,
        Some(Layout::Wikidot)
    );
    run_endpoint!(
        runner,
        page_set_layout,
        input(site_id, editable, SAMPLE_USER_ID, None),
    );
    assert_eq!(layout(&runner, site_id, editable).await, None);
}

#[tokio::test]
async fn forged_actor_cannot_use_administrator_permission() {
    let (mut runner, site_id, _, protected) = setup().await;
    target(&mut runner, site_id, protected, Some(SAMPLE_USER_ID));
    let error = run_endpoint_err!(
        runner,
        page_set_layout,
        input(site_id, protected, ADMIN_USER_ID, Some("wikidot")),
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(layout(&runner, site_id, protected).await, None);
}

#[tokio::test]
async fn body_target_cannot_borrow_header_page_permission() {
    let (mut runner, site_id, editable, protected) = setup().await;
    target(&mut runner, site_id, editable, Some(SAMPLE_USER_ID));
    let error = run_endpoint_err!(
        runner,
        page_set_layout,
        input(site_id, protected, SAMPLE_USER_ID, Some("wikidot")),
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(layout(&runner, site_id, protected).await, None);
}
