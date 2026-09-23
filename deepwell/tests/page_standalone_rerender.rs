//! A standalone rerender recompiles one page without queueing its dependents.
//! Run with a dedicated Redis database: the assertion observes cumulative queue sends.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::page_revision;
use deepwell::services::job::{
    JOB_QUEUE_DELAY, JOB_QUEUE_MAXIMUM_SIZE, JOB_QUEUE_NAME, JOB_QUEUE_PROCESS_TIME,
};
use deepwell::services::page::CreatePage;
use deepwell::services::{PageRevisionService, PageService, TextService};
use deepwell::types::Reference;
use redis::AsyncCommands;
use rsmq_async::{Rsmq, RsmqConnection};
use sea_orm::{ActiveModelTrait, Set};
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
            revision_comments: "Standalone rerender fixture".into(),
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
#[ignore = "requires a dedicated empty Redis database; run explicitly with --ignored"]
async fn standalone_rerender_updates_the_page_without_queueing_dependents() {
    let queue = redis::Client::open(std::env::var("REDIS_URL").unwrap()).unwrap();
    let mut connection = queue.get_multiplexed_async_connection().await.unwrap();
    Rsmq::new_with_connection(connection.clone(), false, Some("rsmq"))
        .await
        .unwrap()
        .create_queue(
            JOB_QUEUE_NAME,
            JOB_QUEUE_PROCESS_TIME,
            JOB_QUEUE_DELAY,
            JOB_QUEUE_MAXIMUM_SIZE,
        )
        .await
        .expect("use a fresh dedicated Redis database");
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    // Rerendering a category template outdates the pages in its category.
    let target_id = import_page(&runner, site_id, "fiction:_template", "Old body").await;
    import_page(&runner, site_id, "fiction:story", "Story").await;
    let target = PageService::get(runner.context(), site_id, Reference::Id(target_id))
        .await
        .unwrap();

    // The stored source changes without a new revision, as after a renderer upgrade.
    let revision = PageRevisionService::get_latest(runner.context(), site_id, target_id)
        .await
        .unwrap();
    let hash = TextService::create(runner.context(), "New body".into())
        .await
        .unwrap();
    page_revision::ActiveModel {
        revision_id: Set(revision.revision_id),
        wikitext_hash: Set(hash.to_vec()),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();

    let rerender = |rerender_type: &str| {
        json!({
            "site_id": site_id,
            "category_id": target.page_category_id,
            "page_id": target_id,
            "rerender_type": rerender_type,
        })
    };
    let sent_before = queue_sent_count(&mut connection).await;
    run_endpoint!(runner, page_rerender, rerender("standalone"));
    assert_eq!(queue_sent_count(&mut connection).await, sent_before);
    let page = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": "fiction:_template", "details": {"compiled_html": true}})
    )
    .unwrap();
    assert!(
        page.compiled_body_html.unwrap().contains("New body"),
        "standalone rerender must recompile the body"
    );

    // A full rerender of the same page does queue its dependents.
    run_endpoint!(runner, page_rerender, rerender("full"));
    assert!(queue_sent_count(&mut connection).await > sent_before);
}
