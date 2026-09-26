//! File replacement with a supplied unchanged name must not conflict with itself.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::prelude::ErrorType;
use deepwell::services::blob::{StartBlobUpload, StartBlobUploadOutput};
use deepwell::services::{BlobService, ServiceContext};
use sea_orm::TransactionTrait;
use serde_json::json;

async fn commit_upload(runner: &TestRunner, data: &[u8]) -> StartBlobUploadOutput {
    let transaction = runner.state().database.begin().await.unwrap();
    let upload = BlobService::start_upload(
        &ServiceContext::new(runner.state(), &transaction),
        StartBlobUpload {
            user_id: ADMIN_USER_ID,
            blob_size: data.len().try_into().unwrap(),
        },
    )
    .await
    .expect("start upload");
    transaction.commit().await.expect("commit pending upload");

    let response = reqwest::Client::new()
        .put(&upload.presign_url)
        .body(data.to_vec())
        .send()
        .await
        .expect("upload fixture bytes");
    assert!(
        response.status().is_success(),
        "S3 PUT: {}",
        response.status()
    );
    upload
}

#[tokio::test]
async fn replacing_bytes_with_same_name_succeeds_but_another_files_name_still_conflicts()
{
    let runner = TestRunner::setup_with_idle_job_workers().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let page_id = run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id": site_id, "user_id": ADMIN_USER_ID,
            "slug": "file-replacement-regression", "title": "File replacement regression",
            "wikitext": "Files", "layout": "wikidot", "revision_comments": "Fixture",
            "bypass_filter": true, "ip_address": "127.0.0.1"
        })
    )
    .page_id;

    let original = commit_upload(&runner, b"original file bytes").await;
    let first = run_endpoint!(
        runner,
        file_create,
        json!({
            "site_id": site_id, "page_id": page_id, "name": "first.txt",
            "uploaded_blob_id": original.pending_blob_id, "user_id": ADMIN_USER_ID,
            "revision_comments": "Original", "bypass_filter": true, "ip_address": "127.0.0.1"
        })
    );
    let other = commit_upload(&runner, b"other file bytes").await;
    let second = run_endpoint!(
        runner,
        file_create,
        json!({
            "site_id": site_id, "page_id": page_id, "name": "second.txt",
            "uploaded_blob_id": other.pending_blob_id, "user_id": ADMIN_USER_ID,
            "revision_comments": "Other", "bypass_filter": true, "ip_address": "127.0.0.1"
        })
    );

    let replacement = commit_upload(&runner, b"replacement file bytes").await;
    let edited = run_endpoint!(
        runner,
        file_edit,
        json!({
            "site_id": site_id, "page_id": page_id, "file_id": first.file_id,
            "last_revision_id": first.file_revision_id, "name": "first.txt",
            "uploaded_blob_id": replacement.pending_blob_id, "user_id": ADMIN_USER_ID,
            "revision_comments": "Replace bytes", "bypass_filter": true,
            "ip_address": "127.0.0.1"
        })
    )
    .expect("replacement should create a revision");
    let stored = run_endpoint!(
        runner,
        file_get,
        json!({
            "site_id": site_id, "page_id": page_id, "file": first.file_id,
            "details": {"data": true}
        })
    )
    .expect("read replacement");
    assert_eq!(stored.file_id, first.file_id);
    assert_eq!(stored.name, "first.txt");
    assert_eq!(stored.revision_id, edited.file_revision_id);
    assert_eq!(stored.data.unwrap().as_ref(), b"replacement file bytes");

    let collision = run_endpoint_err!(
        runner,
        file_edit,
        json!({
            "site_id": site_id, "page_id": page_id, "file_id": first.file_id,
            "last_revision_id": edited.file_revision_id, "name": "second.txt",
            "user_id": ADMIN_USER_ID, "revision_comments": "Collision",
            "bypass_filter": true, "ip_address": "127.0.0.1"
        })
    );
    assert_contains_error!(collision, ErrorType::FileExists);
    let unchanged = run_endpoint!(
        runner,
        file_get,
        json!({
            "site_id": site_id, "page_id": page_id, "file": first.file_id
        })
    )
    .unwrap();
    assert_eq!(unchanged.name, "first.txt");
    assert_eq!(unchanged.revision_id, edited.file_revision_id);
    let other_file = run_endpoint!(
        runner,
        file_get,
        json!({
            "site_id": site_id, "page_id": page_id, "file": second.file_id
        })
    )
    .unwrap();
    assert_eq!(other_file.name, "second.txt");
}
