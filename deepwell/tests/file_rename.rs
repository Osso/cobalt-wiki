//! A filename-only edit must keep the uploaded bytes addressable by the new name.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::{ADMIN_USER_ID, SYSTEM_USER_ID};
use deepwell::services::blob::StartBlobUpload;
use deepwell::services::permission::PermissionService;
use deepwell::services::role::{
    GrantUserRoleInput, InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::{BlobService, RequestContext, ServiceContext};
use deepwell::types::{Action, Permission, Reference, Resource};
use sea_orm::TransactionTrait;
use serde_json::json;

async fn import_attachment_rename_page(runner: &TestRunner, site_id: i64) -> i64 {
    run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id": site_id, "user_id": ADMIN_USER_ID,
            "slug": "attachment-rename-regression", "title": "Attachment rename",
            "wikitext": "Owned rename fixture", "layout": "wikidot",
            "revision_comments": "Fixture", "bypass_filter": true,
            "ip_address": "127.0.0.1"
        })
    )
    .page_id
}

#[tokio::test]
async fn rename_without_reupload_preserves_bytes_and_retires_old_name() {
    let mut runner = TestRunner::setup_with_idle_job_workers().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let page_id = import_attachment_rename_page(&runner, site_id).await;
    let role = RoleService::create(
        runner.context(),
        InternalCreateRoleInput {
            site_id,
            name: "Attachment rename editor".into(),
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
            role_reference: Reference::Id(role.role_id),
            new_permissions: vec![Permission {
                resource_type: Resource::Page,
                resource_category: None,
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
            user_id: ADMIN_USER_ID,
            role_id: role.role_id,
            assigning_user_id: SYSTEM_USER_ID,
            expires_at: None,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    runner.set_request_context(RequestContext {
        site_id: Some(site_id),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(page_id)),
        ..Default::default()
    });
    // A JPEG signature keeps the original MIME type relevant to serving.
    let bytes = b"\xff\xd8\xff\xe0\x00\x10JFIF\x00attachment rename fixture\xff\xd9";
    let transaction = runner.state().database.begin().await.unwrap();
    let upload = BlobService::start_upload(
        &ServiceContext::new(runner.state(), &transaction),
        StartBlobUpload {
            user_id: ADMIN_USER_ID,
            blob_size: bytes.len().try_into().unwrap(),
        },
    )
    .await
    .expect("start fixture upload");
    transaction.commit().await.expect("commit pending upload");
    let response = reqwest::Client::new()
        .put(&upload.presign_url)
        .body(bytes.to_vec())
        .send()
        .await
        .expect("upload owned fixture bytes");
    assert!(
        response.status().is_success(),
        "S3 PUT: {}",
        response.status()
    );
    let created = run_endpoint!(
        runner,
        file_create,
        json!({
            "site_id": site_id, "page_id": page_id, "name": "icon_tyrrell.jpg",
            "uploaded_blob_id": upload.pending_blob_id, "user_id": ADMIN_USER_ID,
            "revision_comments": "Original", "bypass_filter": true,
            "ip_address": "127.0.0.1"
        })
    );
    let page_revision = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": page_id})
    )
    .unwrap();
    run_endpoint!(
        runner,
        page_edit,
        json!({
            "site_id": site_id, "page": page_id, "user_id": ADMIN_USER_ID,
            "last_revision_id": page_revision.revision_id, "wikitext": "[[gallery]]",
            "revision_comments": "Show attachment gallery", "ip_address": "127.0.0.1"
        })
    );
    let gallery_before = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": page_id,
            "details": {"compiled_html": true, "wikitext": true}
        })
    )
    .unwrap();
    assert!(
        gallery_before
            .compiled_body_html
            .as_ref()
            .unwrap()
            .contains("/-/file/attachment-rename-regression/icon_tyrrell.jpg"),
        "fixture gallery must contain the original name before rename"
    );
    let before = run_endpoint!(
        runner,
        file_get,
        json!({"site_id": site_id, "page_id": page_id, "file": "icon_tyrrell.jpg"})
    )
    .expect("original filename lookup");

    let renamed = run_endpoint!(
        runner,
        file_edit,
        json!({
            "site_id": site_id, "page_id": page_id, "file_id": created.file_id,
            "last_revision_id": created.file_revision_id, "name": "icon_zath.jpg",
            "user_id": ADMIN_USER_ID, "revision_comments": "Rename only",
            "bypass_filter": true, "ip_address": "127.0.0.1"
        })
    )
    .expect("rename creates a revision without a new upload");
    let after = run_endpoint!(
        runner,
        file_get,
        json!({
            "site_id": site_id, "page_id": page_id, "file": "icon_zath.jpg",
            "details": {"data": true}
        })
    )
    .expect("serving lookup resolves the renamed file");
    assert_eq!(after.file_id, created.file_id);
    assert_eq!(after.name, "icon_zath.jpg");
    assert_eq!(after.revision_id, renamed.file_revision_id);
    assert_eq!(after.revision_number, before.revision_number + 1);
    assert_eq!(after.size, before.size);
    assert_eq!(after.mime, before.mime);
    assert_eq!(after.s3_hash, before.s3_hash);
    assert_eq!(after.data.unwrap().as_ref(), bytes);
    assert!(
        run_endpoint!(
            runner,
            file_get,
            json!({"site_id": site_id, "page_id": page_id, "file": "icon_tyrrell.jpg"})
        )
        .is_none(),
        "old filename must not remain an alias"
    );
    let revision = run_endpoint!(
        runner,
        file_revision_get,
        json!({
            "site_id": site_id, "page_id": page_id, "file_id": created.file_id,
            "revision_number": after.revision_number
        })
    )
    .expect("read rename revision");
    assert_eq!(revision.name, after.name);
    assert_eq!(revision.changes, ["name"]);
    let gallery_after = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": page_id,
            "details": {"compiled_html": true, "wikitext": true}
        })
    )
    .unwrap();
    let html = gallery_after.compiled_body_html.unwrap();
    assert!(
        html.contains("/-/file/attachment-rename-regression/icon_zath.jpg"),
        "owner gallery must render the new filename after rename: {html}"
    );
    assert!(!html.contains("icon_tyrrell.jpg"), "{html}");
    assert_eq!(gallery_after.wikitext, gallery_before.wikitext);
    assert_eq!(gallery_after.revision_id, gallery_before.revision_id);
    assert_eq!(
        gallery_after.revision_number,
        gallery_before.revision_number
    );
}
