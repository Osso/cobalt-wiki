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
use serde_json::json;

async fn setup() -> (TestRunner, i64, i64, i64) {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let editable = create_page(&runner, site_id, "editable:delete-restore").await;
    let protected = create_page(&runner, site_id, "protected:delete-restore").await;
    let role = RoleService::create(
        runner.context(),
        InternalCreateRoleInput {
            site_id,
            name: "Delete restore editor".into(),
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
    let protected_category_id =
        PageService::get(runner.context(), site_id, Reference::Id(protected))
            .await
            .unwrap()
            .page_category_id;
    let admin_role = RoleService::create(
        runner.context(),
        InternalCreateRoleInput {
            site_id,
            name: "Fixture page administrator".into(),
            description: None,
            is_virtual: false,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    PermissionService::update_permissions_for_role(
        runner.context(),
        UpdateRolePermissionsInput {
            site_id,
            role_reference: Reference::Id(admin_role.role_id),
            new_permissions: [category_id, protected_category_id]
                .into_iter()
                .map(|category_id| Permission {
                    resource_type: Resource::Page,
                    resource_category: Some(Reference::Id(category_id)),
                    action: Action::Edit,
                })
                .collect(),
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
            user_id: ADMIN_USER_ID,
            role_id: admin_role.role_id,
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

fn target(runner: &mut TestRunner, site_id: i64, page_id: i64, actor: i64) {
    runner.set_request_context(RequestContext {
        user_id: Some(actor),
        site_id: Some(site_id),
        page_reference: Some(Reference::Id(page_id)),
        ..Default::default()
    });
}

async fn page_state(runner: &TestRunner, page_id: i64) -> (bool, Option<i64>) {
    let page = PageService::get_direct(runner.context(), page_id, true)
        .await
        .unwrap();
    (page.deleted_at.is_some(), page.latest_revision_id)
}

fn delete_input(
    site_id: i64,
    page_id: i64,
    user_id: i64,
    revision_id: i64,
) -> serde_json::Value {
    json!({
        "site_id": site_id,
        "page": page_id,
        "last_revision_id": revision_id,
        "revision_comments": "Delete",
        "user_id": user_id,
        "ip_address": common::IP_ADDRESS,
    })
}

fn restore_input(site_id: i64, page_id: i64, user_id: i64) -> serde_json::Value {
    json!({
        "site_id": site_id,
        "page_id": page_id,
        "revision_comments": "Restore",
        "user_id": user_id,
        "ip_address": common::IP_ADDRESS,
    })
}

async fn delete_as_admin(runner: &mut TestRunner, site_id: i64, page_id: i64) {
    let revision_id = page_state(runner, page_id).await.1.unwrap();
    target(runner, site_id, page_id, ADMIN_USER_ID);
    run_endpoint!(
        runner,
        page_delete,
        delete_input(site_id, page_id, ADMIN_USER_ID, revision_id)
    );
}

#[tokio::test]
async fn page_edit_allows_delete_and_restore_without_delete_or_create_permission() {
    let (mut runner, site_id, editable, _) = setup().await;
    target(&mut runner, site_id, editable, SAMPLE_USER_ID);
    let before = page_state(&runner, editable).await;
    run_endpoint!(
        runner,
        page_delete,
        delete_input(site_id, editable, SAMPLE_USER_ID, before.1.unwrap())
    );
    let deleted = page_state(&runner, editable).await;
    assert!(deleted.0);
    assert_ne!(deleted.1, before.1);
    run_endpoint!(
        runner,
        page_restore,
        restore_input(site_id, editable, SAMPLE_USER_ID)
    );
    let restored = page_state(&runner, editable).await;
    assert!(!restored.0);
    assert_ne!(restored.1, deleted.1);
}

#[tokio::test]
async fn unauthorized_delete_leaves_page_unchanged() {
    let (mut runner, site_id, _, protected) = setup().await;
    target(&mut runner, site_id, protected, SAMPLE_USER_ID);
    let before = page_state(&runner, protected).await;
    let error = run_endpoint_err!(
        runner,
        page_delete,
        delete_input(site_id, protected, SAMPLE_USER_ID, before.1.unwrap())
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(page_state(&runner, protected).await, before);
}

#[tokio::test]
async fn unauthorized_restore_leaves_deleted_page_unchanged() {
    let (mut runner, site_id, _, protected) = setup().await;
    delete_as_admin(&mut runner, site_id, protected).await;
    target(&mut runner, site_id, protected, SAMPLE_USER_ID);
    let before = page_state(&runner, protected).await;
    let error = run_endpoint_err!(
        runner,
        page_restore,
        restore_input(site_id, protected, SAMPLE_USER_ID)
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(page_state(&runner, protected).await, before);
}

#[tokio::test]
async fn forged_actor_cannot_delete_or_restore() {
    let (mut runner, site_id, editable, _) = setup().await;
    target(&mut runner, site_id, editable, SAMPLE_USER_ID);
    let before = page_state(&runner, editable).await;
    let error = run_endpoint_err!(
        runner,
        page_delete,
        delete_input(site_id, editable, ADMIN_USER_ID, before.1.unwrap())
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(page_state(&runner, editable).await, before);

    delete_as_admin(&mut runner, site_id, editable).await;
    target(&mut runner, site_id, editable, SAMPLE_USER_ID);
    let before = page_state(&runner, editable).await;
    let error = run_endpoint_err!(
        runner,
        page_restore,
        restore_input(site_id, editable, ADMIN_USER_ID)
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(page_state(&runner, editable).await, before);
}

#[tokio::test]
async fn forged_site_cannot_delete_or_restore() {
    let (mut runner, site_id, editable, _) = setup().await;
    target(&mut runner, site_id + 1, editable, SAMPLE_USER_ID);
    let before = page_state(&runner, editable).await;
    let error = run_endpoint_err!(
        runner,
        page_delete,
        delete_input(site_id, editable, SAMPLE_USER_ID, before.1.unwrap())
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(page_state(&runner, editable).await, before);

    delete_as_admin(&mut runner, site_id, editable).await;
    target(&mut runner, site_id + 1, editable, SAMPLE_USER_ID);
    let before = page_state(&runner, editable).await;
    let error = run_endpoint_err!(
        runner,
        page_restore,
        restore_input(site_id, editable, SAMPLE_USER_ID)
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(page_state(&runner, editable).await, before);
}

#[tokio::test]
async fn body_page_cannot_borrow_header_page_edit_for_delete_or_restore() {
    let (mut runner, site_id, editable, protected) = setup().await;
    target(&mut runner, site_id, editable, SAMPLE_USER_ID);
    let before = page_state(&runner, protected).await;
    let error = run_endpoint_err!(
        runner,
        page_delete,
        delete_input(site_id, protected, SAMPLE_USER_ID, before.1.unwrap())
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(page_state(&runner, protected).await, before);

    delete_as_admin(&mut runner, site_id, protected).await;
    target(&mut runner, site_id, editable, SAMPLE_USER_ID);
    let before = page_state(&runner, protected).await;
    let error = run_endpoint_err!(
        runner,
        page_restore,
        restore_input(site_id, protected, SAMPLE_USER_ID)
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(page_state(&runner, protected).await, before);
}
