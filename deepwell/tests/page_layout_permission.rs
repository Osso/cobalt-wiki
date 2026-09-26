#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::{ADMIN_USER_ID, SAMPLE_USER_ID, SYSTEM_USER_ID};
use deepwell::error::ErrorType;
use deepwell::services::RequestContext;
use deepwell::services::page::{CreatePage, PageService};
use deepwell::services::parent::ParentService;
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

fn parents_input(
    site_id: i64,
    child: i64,
    user_id: i64,
    add: &[i64],
    remove: &[i64],
) -> serde_json::Value {
    json!({
        "site_id": site_id,
        "child": child,
        "user_id": user_id,
        "add": add,
        "remove": remove,
    })
}

#[tokio::test]
async fn individual_parent_mutations_cannot_bypass_body_child_authorization() {
    let (mut runner, site_id, editable, protected) = setup().await;
    let parent = create_page(&runner, site_id, "individual:parent").await;
    target(&mut runner, site_id, editable, Some(SAMPLE_USER_ID));
    let input = json!({"site_id":site_id,"child":protected,"parent":parent});
    let error = run_endpoint_err!(runner, parent_set, input.clone());
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert!(parent_ids(&runner, site_id, protected).await.is_empty());
    target(&mut runner, site_id, protected, Some(ADMIN_USER_ID));
    run_endpoint!(runner, parent_set, input.clone());
    target(&mut runner, site_id, editable, Some(SAMPLE_USER_ID));
    let error = run_endpoint_err!(runner, parent_remove, input);
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(parent_ids(&runner, site_id, protected).await, vec![parent]);
}

async fn parent_ids(runner: &TestRunner, site_id: i64, child: i64) -> Vec<i64> {
    let mut ids: Vec<_> =
        ParentService::get_parents(runner.context(), site_id, Reference::Id(child))
            .await
            .unwrap()
            .into_iter()
            .map(|relationship| relationship.parent_page_id)
            .collect();
    ids.sort_unstable();
    ids
}

#[tokio::test]
async fn denied_actor_cannot_add_or_remove_parents() {
    let (mut runner, site_id, editable, protected) = setup().await;
    let other = create_page(&runner, site_id, "other:parent").await;
    target(&mut runner, site_id, protected, Some(ADMIN_USER_ID));
    run_endpoint!(
        runner,
        parent_update,
        parents_input(site_id, protected, ADMIN_USER_ID, &[editable], &[])
    );
    target(&mut runner, site_id, protected, Some(SAMPLE_USER_ID));

    let error = run_endpoint_err!(
        runner,
        parent_update,
        parents_input(site_id, protected, SAMPLE_USER_ID, &[other], &[editable])
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert_eq!(
        parent_ids(&runner, site_id, protected).await,
        vec![editable]
    );
}

#[tokio::test]
async fn forged_actor_cannot_update_parents() {
    let (mut runner, site_id, _, protected) = setup().await;
    let parent = create_page(&runner, site_id, "forged:parent").await;
    target(&mut runner, site_id, protected, Some(SAMPLE_USER_ID));

    let error = run_endpoint_err!(
        runner,
        parent_update,
        parents_input(site_id, protected, ADMIN_USER_ID, &[parent], &[])
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert!(parent_ids(&runner, site_id, protected).await.is_empty());
}

#[tokio::test]
async fn request_site_must_match_parent_update_site() {
    let (mut runner, site_id, editable, _) = setup().await;
    let parent = create_page(&runner, site_id, "site:parent").await;
    target(&mut runner, site_id + 1, editable, Some(SAMPLE_USER_ID));

    let error = run_endpoint_err!(
        runner,
        parent_update,
        parents_input(site_id, editable, SAMPLE_USER_ID, &[parent], &[])
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert!(parent_ids(&runner, site_id, editable).await.is_empty());
}

#[tokio::test]
async fn body_child_cannot_borrow_header_page_permission_for_parents() {
    let (mut runner, site_id, editable, protected) = setup().await;
    let parent = create_page(&runner, site_id, "body:parent").await;
    target(&mut runner, site_id, editable, Some(SAMPLE_USER_ID));

    let error = run_endpoint_err!(
        runner,
        parent_update,
        parents_input(site_id, protected, SAMPLE_USER_ID, &[parent], &[])
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert!(parent_ids(&runner, site_id, protected).await.is_empty());
}

#[tokio::test]
async fn authorized_actor_can_update_plural_parents_despite_different_header_page() {
    let (mut runner, site_id, editable, protected) = setup().await;
    let first = create_page(&runner, site_id, "first:parent").await;
    let second = create_page(&runner, site_id, "second:parent").await;
    let third = create_page(&runner, site_id, "third:parent").await;
    target(&mut runner, site_id, protected, Some(SAMPLE_USER_ID));

    let created = run_endpoint!(
        runner,
        parent_update,
        parents_input(site_id, editable, SAMPLE_USER_ID, &[first, second], &[])
    );
    assert_eq!(created.added, Some(vec![first, second]));
    assert_eq!(created.removed, Some(vec![]));
    assert_eq!(
        parent_ids(&runner, site_id, editable).await,
        vec![first, second]
    );

    let changed = run_endpoint!(
        runner,
        parent_update,
        parents_input(
            site_id,
            editable,
            SAMPLE_USER_ID,
            &[third],
            &[first, second]
        )
    );
    assert_eq!(changed.added, Some(vec![third]));
    assert_eq!(changed.removed, Some(vec![true, true]));
    assert_eq!(parent_ids(&runner, site_id, editable).await, vec![third]);

    let unchanged = run_endpoint!(
        runner,
        parent_update,
        json!({ "site_id": site_id, "child": editable, "user_id": SAMPLE_USER_ID })
    );
    assert_eq!(unchanged.added, None);
    assert_eq!(unchanged.removed, None);
    assert_eq!(parent_ids(&runner, site_id, editable).await, vec![third]);
}
