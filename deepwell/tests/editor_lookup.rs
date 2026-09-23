//! Editor attachment lookup is scoped to the trusted current page and editor.
#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::{ADMIN_USER_ID, SYSTEM_USER_ID};
use deepwell::error::ErrorType;
use deepwell::models::{file, file_revision};
use deepwell::services::PageService;
use deepwell::services::RequestContext;
use deepwell::services::category::CategoryService;
use deepwell::services::permission::{CheckPermissionContext, PermissionService};
use deepwell::services::role::{
    InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::types::{Action, FileRevisionType, Permission, Reference, Resource};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::json;

async fn setup() -> (TestRunner, i64) {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    (runner, site_id)
}

fn target(
    runner: &mut TestRunner,
    site_id: i64,
    page: Reference<'static>,
    user: Option<i64>,
) {
    runner.set_request_context(RequestContext {
        site_id: Some(site_id),
        page_reference: Some(page),
        user_id: user,
        ..Default::default()
    });
}

async fn page(runner: &TestRunner, site_id: i64, slug: &str) -> i64 {
    run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id": site_id, "user_id": ADMIN_USER_ID, "slug": slug,
            "title": slug, "wikitext": "Attachment fixture", "alt_title": null,
            "layout": "wikidot", "revision_comments": "Fixture", "bypass_filter": true,
            "ip_address": common::IP_ADDRESS
        })
    )
    .page_id
}

async fn attachment(
    runner: &TestRunner,
    site_id: i64,
    page_id: i64,
    name: &str,
    mime: &str,
    deleted: bool,
) -> i64 {
    let file = file::ActiveModel {
        site_id: Set(site_id),
        page_id: Set(page_id),
        name: Set(name.to_owned()),
        from_wikidot: Set(false),
        deleted_at: Set(deleted.then(deepwell::utils::now)),
        ..Default::default()
    }
    .insert(runner.context().transaction())
    .await
    .unwrap();
    revision(runner, site_id, page_id, file.file_id, name, mime, 0).await;
    file.file_id
}

async fn revision(
    runner: &TestRunner,
    site_id: i64,
    page_id: i64,
    file_id: i64,
    name: &str,
    mime: &str,
    number: i32,
) {
    file_revision::ActiveModel {
        site_id: Set(site_id),
        page_id: Set(page_id),
        file_id: Set(file_id),
        user_id: Set(ADMIN_USER_ID),
        name: Set(name.to_owned()),
        mime: Set(mime.to_owned()),
        revision_type: Set(if number == 0 {
            FileRevisionType::Create
        } else {
            FileRevisionType::Regular
        }),
        revision_number: Set(number),
        s3_hash: Set(vec![1; 64]),
        size: Set(1),
        changes: Set(if number == 0 {
            vec!["page", "name", "blob", "mime"]
        } else {
            vec!["mime"]
        }
        .into_iter()
        .map(str::to_owned)
        .collect()),
        comments: Set("Fixture".into()),
        hidden: Set(vec![]),
        ..Default::default()
    }
    .insert(runner.context().transaction())
    .await
    .unwrap();
}

#[tokio::test]
async fn editor_receives_only_current_image_names_from_active_files() {
    let (mut runner, site_id) = setup().await;
    let page_id = page(&runner, site_id, "editor:images").await;
    attachment(
        &runner,
        site_id,
        page_id,
        "hero image.png",
        "image/png",
        false,
    )
    .await;
    let updated =
        attachment(&runner, site_id, page_id, "old.png", "image/png", false).await;
    revision(
        &runner,
        site_id,
        page_id,
        updated,
        "old.png",
        "text/plain",
        1,
    )
    .await;
    attachment(&runner, site_id, page_id, "notes.txt", "text/plain", false).await;
    attachment(&runner, site_id, page_id, "deleted.gif", "image/gif", true).await;
    target(
        &mut runner,
        site_id,
        Reference::Id(page_id),
        Some(ADMIN_USER_ID),
    );

    let output = run_endpoint!(runner, editor_attachments);
    assert_eq!(
        serde_json::to_value(output).unwrap(),
        json!([{"name": "hero image.png"}])
    );
}

#[tokio::test]
async fn anonymous_and_missing_pages_fail_instead_of_returning_files() {
    let (mut runner, site_id) = setup().await;
    let page_id = page(&runner, site_id, "editor:private").await;
    attachment(&runner, site_id, page_id, "private.png", "image/png", false).await;
    target(&mut runner, site_id, Reference::Id(page_id), None);
    let denied = run_endpoint_err!(runner, editor_attachments);
    assert_contains_error!(denied, ErrorType::PermissionDenied);

    target(
        &mut runner,
        site_id,
        Reference::Slug("editor:absent".into()),
        Some(ADMIN_USER_ID),
    );
    let missing = run_endpoint_err!(runner, editor_attachments);
    assert_contains_error!(missing, ErrorType::PageNotFound);
}

#[tokio::test]
async fn private_category_rejects_anonymous_view_even_with_forged_editor_identity() {
    let (mut runner, site_id) = setup().await;
    let category_id =
        CategoryService::get_or_create(runner.context(), site_id, "private-lookup")
            .await
            .unwrap()
            .category_id;
    let role_id = RoleService::create(
        runner.context(),
        InternalCreateRoleInput {
            site_id,
            name: "private-lookup-author".into(),
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
    let page_id = page(&runner, site_id, "private-lookup:application").await;
    attachment(&runner, site_id, page_id, "secret.png", "image/png", false).await;
    target(&mut runner, site_id, Reference::Id(page_id), None);
    let visible = PageService::check_user_permission(
        runner.context(),
        &CheckPermissionContext {
            user_id: None,
            site_id,
            page_reference: Some(Reference::Id(page_id)),
        },
        Action::View,
    )
    .await
    .unwrap();
    assert!(!visible, "anonymous viewer must not see private category");
    let denied = run_endpoint_err!(
        runner,
        editor_attachments,
        json!({
            "site_id": site_id, "page_id": page_id, "user_id": ADMIN_USER_ID
        })
    );
    assert_contains_error!(denied, ErrorType::PermissionDenied);
}

#[tokio::test]
async fn forged_target_cannot_select_another_page_or_site() {
    let (mut runner, site_id) = setup().await;
    let current = page(&runner, site_id, "editor:current").await;
    let other = page(&runner, site_id, "editor:other").await;
    attachment(&runner, site_id, current, "current.png", "image/png", false).await;
    attachment(&runner, site_id, other, "other.png", "image/png", false).await;
    target(
        &mut runner,
        site_id,
        Reference::Id(current),
        Some(ADMIN_USER_ID),
    );
    let output = run_endpoint!(
        runner,
        editor_attachments,
        json!({
            "site_id": site_id + 1, "page_id": other, "page": other,
            "user_id": ADMIN_USER_ID
        })
    );
    assert_eq!(
        serde_json::to_value(output).unwrap(),
        json!([{"name": "current.png"}])
    );

    target(
        &mut runner,
        site_id + 1,
        Reference::Id(current),
        Some(ADMIN_USER_ID),
    );
    let missing = run_endpoint_err!(
        runner,
        editor_attachments,
        json!({
            "site_id": site_id, "page_id": current, "user_id": ADMIN_USER_ID
        })
    );
    assert_contains_error!(missing, ErrorType::PageNotFound);
}
