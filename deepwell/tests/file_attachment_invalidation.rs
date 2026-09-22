//! Creating an attachment must not invalidate links to its existing owner page.
//! Run with a dedicated Redis database: the assertion observes cumulative queue sends.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::site;
use deepwell::services::blob::StartBlobUpload;
use deepwell::services::page::CreatePage;
use deepwell::services::{BlobService, LinkService, PageService, ServiceContext};
use deepwell::types::{ConnectionType, Reference};
use redis::AsyncCommands;
use sea_orm::{ActiveModelTrait, Set, TransactionTrait};
use serde_json::json;

async fn import_page(runner: &TestRunner, site_id: i64, slug: &str, source: &str) -> i64 {
    PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            slug: slug.into(),
            title: slug.into(),
            wikitext: source.into(),
            alt_title: None,
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Attachment invalidation fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("import fixture page")
    .page_id
}

async fn queue_sent_count(connection: &mut redis::aio::MultiplexedConnection) -> u64 {
    let count: Option<u64> = connection
        .hget("rsmq:job:Q", "totalsent")
        .await
        .expect("read isolated queue sent counter");
    count.expect("test runner created isolated job queue")
}

#[tokio::test]
async fn attachment_on_existing_page_does_not_queue_ordinary_link_dependents() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let owner_id =
        import_page(&runner, site_id, "attachment-owner", "Owner unchanged").await;
    let nav_id =
        import_page(&runner, site_id, "attachment-nav", "[[[attachment-owner]]]").await;
    site::ActiveModel {
        site_id: Set(site_id),
        top_bar_page: Set("attachment-nav".into()),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .expect("configure site navigation");
    let links =
        LinkService::get_to(runner.context(), owner_id, Some(&[ConnectionType::Link]))
            .await
            .expect("read link dependents");
    assert!(
        links
            .connections
            .iter()
            .any(|link| link.from_page_id == nav_id),
        "navigation must link to the existing attachment owner"
    );

    let queue = redis::Client::open(std::env::var("REDIS_URL").unwrap()).unwrap();
    let mut connection = queue.get_multiplexed_async_connection().await.unwrap();
    let sent_before = queue_sent_count(&mut connection).await;

    let data = b"attachment invalidation regression: real S3 upload";
    // Production commits blob_upload before PUT and file_create. Finalization
    // uses another transaction, so an uncommitted upload fixture is invisible.
    let upload_txn = runner.state().database.begin().await.unwrap();
    let upload = BlobService::start_upload(
        &ServiceContext::new(runner.state(), &upload_txn),
        StartBlobUpload {
            user_id: ADMIN_USER_ID,
            blob_size: data.len().try_into().unwrap(),
        },
    )
    .await
    .expect("create upload in its own request transaction");
    upload_txn
        .commit()
        .await
        .expect("commit upload before sending bytes");
    let response = reqwest::Client::new()
        .put(&upload.presign_url)
        .body(data.to_vec())
        .send()
        .await
        .expect("upload fixture bytes to local S3");
    assert!(
        response.status().is_success(),
        "local S3 PUT: {}",
        response.status()
    );
    let created = run_endpoint!(
        runner,
        file_create,
        json!({
            "site_id": site_id,
            "page_id": owner_id,
            "name": "attachment-regression.txt",
            "uploaded_blob_id": upload.pending_blob_id,
            "revision_comments": "Attachment invalidation regression",
            "user_id": ADMIN_USER_ID,
            "bypass_filter": true,
            "ip_address": "127.0.0.1"
        })
    );
    let stored = run_endpoint!(
        runner,
        file_get,
        json!({"site_id": site_id, "page_id": owner_id, "file": created.file_id})
    )
    .expect("created attachment should be readable");
    assert_eq!(stored.page_id, owner_id);
    assert_eq!(stored.name, "attachment-regression.txt");
    assert_eq!(stored.size, data.len() as i64);
    let owner = PageService::get(runner.context(), site_id, Reference::Id(owner_id))
        .await
        .expect("owner page remains present");
    assert_eq!(owner.slug, "attachment-owner");

    let sent_after = queue_sent_count(&mut connection).await;
    assert_eq!(
        sent_after, sent_before,
        "creating a file must not enqueue rerenders for unchanged page links"
    );
}
