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
    let editable = page(&runner, site_id, "files-editable:fixture").await;
    let protected = page(&runner, site_id, "files-protected:fixture").await;
    let role = RoleService::create(
        runner.context(),
        InternalCreateRoleInput {
            site_id,
            name: "File editor".into(),
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

fn actor(runner: &mut TestRunner, site_id: i64, header_page: i64, user_id: i64) {
    runner.set_request_context(RequestContext {
        site_id: Some(site_id),
        user_id: Some(user_id),
        page_reference: Some(Reference::Id(header_page)),
        ..Default::default()
    });
}

async fn upload(runner: &TestRunner, user_id: i64) -> String {
    let txn = runner.state().database.begin().await.unwrap();
    let pending = BlobService::start_upload(
        &ServiceContext::new(runner.state(), &txn),
        StartBlobUpload {
            user_id,
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
    pending.pending_blob_id
}

fn create_input(site_id: i64, page_id: i64, user_id: i64, blob: &str) -> Value {
    json!({
        "site_id": site_id, "page_id": page_id, "user_id": user_id,
        "name": "fixture.txt", "uploaded_blob_id": blob,
        "revision_comments": "Fixture", "bypass_filter": true,
        "ip_address": "127.0.0.1"
    })
}

async fn create(runner: &TestRunner, site_id: i64, page_id: i64) -> (i64, i64) {
    let blob = upload(runner, ADMIN_USER_ID).await;
    let output = run_endpoint!(
        runner,
        file_create,
        create_input(site_id, page_id, ADMIN_USER_ID, &blob)
    );
    (output.file_id, output.file_revision_id)
}

async fn unchanged(runner: &TestRunner, file_id: i64, page_id: i64, revision_id: i64) {
    let file = FileService::get_direct(runner.context(), file_id, true)
        .await
        .unwrap();
    assert_eq!(file.page_id, page_id);
    assert_eq!(file.deleted_at, None);
    let revision = run_endpoint!(
        runner,
        file_get,
        json!({"site_id": file.site_id, "page_id": page_id, "file": file_id})
    )
    .unwrap();
    assert_eq!(revision.revision_id, revision_id);
}

#[tokio::test]
async fn denied_mutations_leave_file_unchanged() {
    let (mut runner, site_id, editable, protected) = setup().await;
    actor(&mut runner, site_id, protected, ADMIN_USER_ID);
    let (file_id, revision_id) = create(&runner, site_id, protected).await;
    actor(&mut runner, site_id, editable, SAMPLE_USER_ID);
    let blob = upload(&runner, ADMIN_USER_ID).await;
    let requests = [
        (
            "file_create",
            create_input(site_id, protected, SAMPLE_USER_ID, &blob),
        ),
        (
            "file_edit",
            json!({"site_id":site_id,"page_id":protected,"file_id":file_id,"user_id":SAMPLE_USER_ID,"last_revision_id":revision_id,"name":"changed.txt","revision_comments":"Edit","ip_address":"127.0.0.1"}),
        ),
        (
            "file_move",
            json!({"site_id":site_id,"current_page_id":protected,"destination_page":editable,"file_id":file_id,"user_id":SAMPLE_USER_ID,"last_revision_id":revision_id,"revision_comments":"Move"}),
        ),
        (
            "file_rollback",
            json!({"site_id":site_id,"page_id":protected,"file":file_id,"user_id":SAMPLE_USER_ID,"last_revision_id":revision_id,"revision_number":1,"revision_comments":"Rollback","ip_address":"127.0.0.1"}),
        ),
        (
            "file_delete",
            json!({"site_id":site_id,"page_id":protected,"file":file_id,"user_id":SAMPLE_USER_ID,"last_revision_id":revision_id,"revision_comments":"Delete"}),
        ),
    ];
    for (operation, input) in requests {
        let error = match operation {
            "file_create" => run_endpoint_err!(runner, file_create, input),
            "file_edit" => run_endpoint_err!(runner, file_edit, input),
            "file_move" => run_endpoint_err!(runner, file_move, input),
            "file_rollback" => run_endpoint_err!(runner, file_rollback, input),
            "file_delete" => run_endpoint_err!(runner, file_delete, input),
            _ => unreachable!(),
        };
        assert_contains_error!(error, ErrorType::PermissionDenied);
        unchanged(&runner, file_id, protected, revision_id).await;
    }
    actor(&mut runner, site_id, protected, ADMIN_USER_ID);
    run_endpoint!(
        runner,
        file_delete,
        json!({"site_id":site_id,"page_id":protected,"file":file_id,"user_id":ADMIN_USER_ID,"last_revision_id":revision_id,"revision_comments":"Delete"})
    );
    actor(&mut runner, site_id, editable, SAMPLE_USER_ID);
    let error = run_endpoint_err!(
        runner,
        file_restore,
        json!({"site_id":site_id,"page_id":protected,"file_id":file_id,"user_id":SAMPLE_USER_ID,"revision_comments":"Restore"})
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert!(
        FileService::get_direct(runner.context(), file_id, true)
            .await
            .unwrap()
            .deleted_at
            .is_some()
    );
}

#[tokio::test]
async fn authorized_lifecycle_and_destination_denial() {
    let (mut runner, site_id, editable, protected) = setup().await;
    actor(&mut runner, site_id, protected, ADMIN_USER_ID);
    let (file_id, initial) = create(&runner, site_id, protected).await;
    actor(&mut runner, site_id, protected, SAMPLE_USER_ID);
    let error = run_endpoint_err!(
        runner,
        file_move,
        json!({"site_id":site_id,"current_page_id":protected,"destination_page":editable,"file_id":file_id,"user_id":SAMPLE_USER_ID,"last_revision_id":initial,"revision_comments":"Move"})
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    unchanged(&runner, file_id, protected, initial).await;

    let blob = upload(&runner, SAMPLE_USER_ID).await;
    let mut sample_input = create_input(site_id, editable, SAMPLE_USER_ID, &blob);
    sample_input["name"] = json!("sample.txt");
    let uploaded = run_endpoint!(runner, file_create, sample_input);
    let destination = page(&runner, site_id, "files-editable:destination").await;
    let relocated = run_endpoint!(runner, file_move, json!({"site_id":site_id,"current_page_id":editable,"destination_page":destination,"file_id":uploaded.file_id,"user_id":SAMPLE_USER_ID,"last_revision_id":uploaded.file_revision_id,"revision_comments":"Move"})).unwrap();
    assert_eq!(
        FileService::get_direct(runner.context(), uploaded.file_id, false)
            .await
            .unwrap()
            .page_id,
        destination
    );
    assert_eq!(relocated.file_revision_number, 1);
    let original_revision = run_endpoint!(
        runner,
        file_revision_get,
        json!({"site_id":site_id,"page_id":editable,"file_id":uploaded.file_id,"revision_number":0})
    )
    .unwrap();
    let move_revision = run_endpoint!(
        runner,
        file_revision_get,
        json!({"site_id":site_id,"page_id":destination,"file_id":uploaded.file_id,"revision_number":1})
    )
    .unwrap();
    assert_eq!(original_revision.revision_id, uploaded.file_revision_id);
    assert_eq!(move_revision.revision_id, relocated.file_revision_id);
    assert_ne!(original_revision.revision_id, move_revision.revision_id);
    actor(&mut runner, site_id, destination, SAMPLE_USER_ID);
    run_endpoint!(
        runner,
        file_delete,
        json!({"site_id":site_id,"page_id":destination,"file":uploaded.file_id,"user_id":SAMPLE_USER_ID,"last_revision_id":relocated.file_revision_id,"revision_comments":"Delete"})
    );
    actor(&mut runner, site_id, protected, SAMPLE_USER_ID);
    let error = run_endpoint_err!(
        runner,
        file_restore,
        json!({"site_id":site_id,"page_id":destination,"new_page":protected,"file_id":uploaded.file_id,"user_id":SAMPLE_USER_ID,"revision_comments":"Restore"})
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    assert!(
        FileService::get_direct(runner.context(), uploaded.file_id, true)
            .await
            .unwrap()
            .deleted_at
            .is_some()
    );
    run_endpoint!(
        runner,
        file_restore,
        json!({"site_id":site_id,"page_id":destination,"file_id":uploaded.file_id,"user_id":SAMPLE_USER_ID,"revision_comments":"Restore"})
    );
    assert!(
        FileService::get_direct(runner.context(), uploaded.file_id, true)
            .await
            .unwrap()
            .deleted_at
            .is_none()
    );
    actor(&mut runner, site_id, protected, ADMIN_USER_ID);
    let (editable_file, first) = create(&runner, site_id, editable).await;
    actor(&mut runner, site_id, protected, SAMPLE_USER_ID);
    let edited = run_endpoint!(runner, file_edit, json!({"site_id":site_id,"page_id":editable,"file_id":editable_file,"user_id":SAMPLE_USER_ID,"last_revision_id":first,"name":"renamed.txt","revision_comments":"Edit","ip_address":"127.0.0.1"})).unwrap();
    let rolled_back = run_endpoint!(runner, file_rollback, json!({"site_id":site_id,"page_id":editable,"file":editable_file,"user_id":SAMPLE_USER_ID,"last_revision_id":edited.file_revision_id,"revision_number":1,"revision_comments":"Rollback","ip_address":"127.0.0.1"})).unwrap();
    assert_eq!(
        run_endpoint!(
            runner,
            file_get,
            json!({"site_id":site_id,"page_id":editable,"file":editable_file})
        )
        .unwrap()
        .name,
        "fixture.txt"
    );
    let error = run_endpoint_err!(
        runner,
        file_move,
        json!({"site_id":site_id,"current_page_id":editable,"destination_page":protected,"file_id":editable_file,"user_id":SAMPLE_USER_ID,"last_revision_id":rolled_back.file_revision_id,"revision_comments":"Move"})
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    unchanged(
        &runner,
        editable_file,
        editable,
        rolled_back.file_revision_id,
    )
    .await;
    actor(&mut runner, site_id, protected, ADMIN_USER_ID);
    let moved = run_endpoint!(runner, file_move, json!({"site_id":site_id,"current_page_id":editable,"destination_page":protected,"file_id":editable_file,"user_id":ADMIN_USER_ID,"last_revision_id":rolled_back.file_revision_id,"name":"moved.txt","revision_comments":"Move"})).unwrap();
    assert_eq!(
        FileService::get_direct(runner.context(), editable_file, false)
            .await
            .unwrap()
            .page_id,
        protected
    );
    let deleted = run_endpoint!(
        runner,
        file_delete,
        json!({"site_id":site_id,"page_id":protected,"file":editable_file,"user_id":ADMIN_USER_ID,"last_revision_id":moved.file_revision_id,"revision_comments":"Delete"})
    );
    assert!(
        FileService::get_direct(runner.context(), editable_file, true)
            .await
            .unwrap()
            .deleted_at
            .is_some()
    );
    actor(&mut runner, site_id, editable, SAMPLE_USER_ID);
    let mismatch = run_endpoint_err!(
        runner,
        file_restore,
        json!({"site_id":site_id,"page_id":editable,"file_id":editable_file,"user_id":SAMPLE_USER_ID,"revision_comments":"Restore"})
    );
    assert_contains_error!(mismatch, ErrorType::PermissionDenied);
    actor(&mut runner, site_id, protected, ADMIN_USER_ID);
    let restored = run_endpoint!(
        runner,
        file_restore,
        json!({"site_id":site_id,"page_id":protected,"file_id":editable_file,"user_id":ADMIN_USER_ID,"revision_comments":"Restore"})
    );
    assert_eq!(
        restored.file_revision_number,
        deleted.file_revision_number + 1
    );
    assert!(
        FileService::get_direct(runner.context(), editable_file, true)
            .await
            .unwrap()
            .deleted_at
            .is_none()
    );
}

#[tokio::test]
async fn forged_identity_and_cross_page_targets_are_rejected() {
    let (mut runner, site_id, editable, protected) = setup().await;
    actor(&mut runner, site_id, protected, ADMIN_USER_ID);
    let (file_id, revision_id) = create(&runner, site_id, protected).await;
    actor(&mut runner, site_id, editable, SAMPLE_USER_ID);
    let blob = upload(&runner, ADMIN_USER_ID).await;
    let error = run_endpoint_err!(
        runner,
        file_create,
        create_input(site_id, editable, ADMIN_USER_ID, &blob)
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    let error = run_endpoint_err!(
        runner,
        file_edit,
        json!({"site_id":site_id,"page_id":editable,"file_id":file_id,"user_id":SAMPLE_USER_ID,"last_revision_id":revision_id,"name":"stolen.txt","revision_comments":"Edit","ip_address":"127.0.0.1"})
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    let error = run_endpoint_err!(
        runner,
        file_move,
        json!({"site_id":site_id,"current_page_id":editable,"destination_page":protected,"file_id":file_id,"user_id":SAMPLE_USER_ID,"last_revision_id":revision_id,"revision_comments":"Move"})
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    let error = run_endpoint_err!(
        runner,
        file_delete,
        json!({"site_id":site_id,"page_id":editable,"file":file_id,"user_id":SAMPLE_USER_ID,"last_revision_id":revision_id,"revision_comments":"Delete"})
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    let error = run_endpoint_err!(
        runner,
        file_rollback,
        json!({"site_id":site_id,"page_id":editable,"file":file_id,"user_id":SAMPLE_USER_ID,"last_revision_id":revision_id,"revision_number":1,"revision_comments":"Rollback","ip_address":"127.0.0.1"})
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    actor(&mut runner, site_id + 1, editable, SAMPLE_USER_ID);
    let error = run_endpoint_err!(
        runner,
        file_create,
        create_input(site_id, editable, SAMPLE_USER_ID, &blob)
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    unchanged(&runner, file_id, protected, revision_id).await;
}
