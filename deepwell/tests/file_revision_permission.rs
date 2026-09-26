#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::{ADMIN_USER_ID, SAMPLE_USER_ID, SYSTEM_USER_ID};
use deepwell::error::ErrorType;
use deepwell::services::blob::{BlobService, StartBlobUpload};
use deepwell::services::page::{CreatePage, PageService};
use deepwell::services::permission::PermissionService;
use deepwell::services::role::{
    GrantUserRoleInput, InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::{FileService, RequestContext, ServiceContext};
use deepwell::types::{Action, Permission, Reference, Resource};
use sea_orm::TransactionTrait;
use serde_json::{Value, json};

async fn page(runner: &TestRunner, site_id: i64, slug: &str) -> i64 {
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

async fn setup() -> (TestRunner, i64, i64, i64) {
    let runner = TestRunner::setup_with_idle_job_workers().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let editable = page(&runner, site_id, "files-editable:history").await;
    let protected = page(&runner, site_id, "files-protected:history").await;
    let role = RoleService::create(
        runner.context(),
        InternalCreateRoleInput {
            site_id,
            name: "History editor".into(),
            description: None,
            is_virtual: false,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    let category = PageService::get(runner.context(), site_id, Reference::Id(editable))
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
                resource_category: Some(Reference::Id(category)),
                action: Action::Edit,
            }],
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    for user_id in [SAMPLE_USER_ID, ADMIN_USER_ID] {
        RoleService::grant_role_to_user(
            runner.context(),
            GrantUserRoleInput {
                site_id,
                user_id,
                role_id: role.role_id,
                assigning_user_id: SYSTEM_USER_ID,
                expires_at: None,
                ip_address: common::IP_ADDRESS,
            },
        )
        .await
        .unwrap();
    }
    (runner, site_id, editable, protected)
}

fn actor(runner: &mut TestRunner, site_id: i64, page_id: i64, user_id: i64) {
    runner.set_request_context(RequestContext {
        site_id: Some(site_id),
        user_id: Some(user_id),
        page_reference: Some(Reference::Id(page_id)),
        ..Default::default()
    });
}

async fn create(runner: &TestRunner, site_id: i64, page_id: i64) -> (i64, i64) {
    let txn = runner.state().database.begin().await.unwrap();
    let pending = BlobService::start_upload(
        &ServiceContext::new(runner.state(), &txn),
        StartBlobUpload {
            user_id: ADMIN_USER_ID,
            blob_size: 5,
        },
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();
    assert!(
        reqwest::Client::new()
            .put(pending.presign_url)
            .body(b"hello".to_vec())
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    let output = run_endpoint!(
        runner,
        file_create,
        json!({
            "site_id": site_id, "page_id": page_id, "user_id": ADMIN_USER_ID,
            "name": "fixture.txt", "uploaded_blob_id": pending.pending_blob_id,
            "revision_comments": "Private history", "bypass_filter": true,
            "ip_address": "127.0.0.1"
        })
    );
    (output.file_id, output.file_revision_id)
}

fn range(file_id: i64) -> Value {
    json!({"file_id":file_id,"revision_number":-1,"revision_direction":"before","limit":20})
}

fn detail(site_id: i64, page_id: i64, file_id: i64) -> Value {
    json!({"site_id":site_id,"page_id":page_id,"file_id":file_id,"revision_number":0})
}

#[tokio::test]
async fn denied_history_cannot_expose_revisions_or_count_with_forged_page() {
    let (mut runner, site_id, editable, protected) = setup().await;
    actor(&mut runner, site_id, protected, ADMIN_USER_ID);
    let (file_id, _) = create(&runner, site_id, protected).await;
    actor(&mut runner, site_id, editable, SAMPLE_USER_ID);
    assert_contains_error!(
        run_endpoint_err!(runner, file_revision_range, range(file_id)),
        ErrorType::PermissionDenied
    );
    assert_contains_error!(
        run_endpoint_err!(
            runner,
            file_revision_get,
            detail(site_id, protected, file_id)
        ),
        ErrorType::PermissionDenied
    );
    assert_contains_error!(
        run_endpoint_err!(
            runner,
            file_revision_count,
            json!({"site_id":site_id,"page_id":protected,"file":file_id})
        ),
        ErrorType::PermissionDenied
    );
    assert_contains_error!(
        run_endpoint_err!(
            runner,
            file_revision_get,
            detail(site_id, editable, file_id)
        ),
        ErrorType::PermissionDenied
    );
    actor(&mut runner, site_id + 1, editable, ADMIN_USER_ID);
    assert_contains_error!(
        run_endpoint_err!(runner, file_revision_range, range(file_id)),
        ErrorType::PermissionDenied
    );
}

#[tokio::test]
async fn edit_grant_reads_active_and_deleted_history() {
    let (mut runner, site_id, editable, _) = setup().await;
    actor(&mut runner, site_id, editable, ADMIN_USER_ID);
    let (file_id, revision_id) = create(&runner, site_id, editable).await;
    actor(&mut runner, site_id, editable, SAMPLE_USER_ID);
    let revisions = run_endpoint!(runner, file_revision_range, range(file_id));
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].revision_id, revision_id);
    assert_eq!(
        run_endpoint!(
            runner,
            file_revision_get,
            detail(site_id, editable, file_id)
        )
        .unwrap()
        .revision_id,
        revision_id
    );
    assert_eq!(
        run_endpoint!(
            runner,
            file_revision_count,
            json!({"site_id":site_id,"page_id":editable,"file":file_id})
        )
        .revision_count
        .get(),
        1
    );
    actor(&mut runner, site_id, editable, ADMIN_USER_ID);
    run_endpoint!(
        runner,
        file_delete,
        json!({"site_id":site_id,"page_id":editable,"file":file_id,"user_id":ADMIN_USER_ID,"last_revision_id":revision_id,"revision_comments":"Delete"})
    );
    assert!(
        FileService::get_direct(runner.context(), file_id, true)
            .await
            .unwrap()
            .deleted_at
            .is_some()
    );
    actor(&mut runner, site_id, editable, SAMPLE_USER_ID);
    assert_eq!(
        run_endpoint!(runner, file_revision_range, range(file_id)).len(),
        2
    );
    assert_eq!(
        run_endpoint!(
            runner,
            file_revision_get,
            detail(site_id, editable, file_id)
        )
        .unwrap()
        .revision_id,
        revision_id
    );
}

#[tokio::test]
async fn moved_file_history_follows_current_owner_not_historical_page() {
    let (mut runner, site_id, editable, protected) = setup().await;
    actor(&mut runner, site_id, editable, ADMIN_USER_ID);
    let (file_id, initial) = create(&runner, site_id, editable).await;
    let moved = run_endpoint!(runner, file_move, json!({"site_id":site_id,"current_page_id":editable,"destination_page":protected,"file_id":file_id,"user_id":ADMIN_USER_ID,"last_revision_id":initial,"revision_comments":"Move"})).unwrap();
    actor(&mut runner, site_id, editable, SAMPLE_USER_ID);
    assert_contains_error!(
        run_endpoint_err!(runner, file_revision_range, range(file_id)),
        ErrorType::PermissionDenied
    );
    assert_contains_error!(
        run_endpoint_err!(
            runner,
            file_revision_get,
            detail(site_id, editable, file_id)
        ),
        ErrorType::PermissionDenied
    );
    actor(&mut runner, site_id, protected, ADMIN_USER_ID);
    assert_eq!(
        run_endpoint!(runner, file_revision_range, range(file_id)).len(),
        2
    );
    assert_eq!(
        run_endpoint!(
            runner,
            file_revision_get,
            detail(site_id, editable, file_id)
        )
        .unwrap()
        .revision_id,
        initial
    );
    let mut moved_detail = detail(site_id, protected, file_id);
    moved_detail["revision_number"] = json!(1);
    assert_eq!(
        run_endpoint!(runner, file_revision_get, moved_detail)
            .unwrap()
            .revision_id,
        moved.file_revision_id
    );
}
