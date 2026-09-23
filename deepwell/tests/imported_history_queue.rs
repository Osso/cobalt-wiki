//! Importing source history must not schedule page or navigation rerenders.
//! Run explicitly against a fresh, dedicated Redis database.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::site;
use deepwell::services::job::{
    JOB_QUEUE_DELAY, JOB_QUEUE_MAXIMUM_SIZE, JOB_QUEUE_NAME, JOB_QUEUE_PROCESS_TIME,
};
use redis::AsyncCommands;
use rsmq_async::{Rsmq, RsmqConnection};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;

async fn sent_count(connection: &mut redis::aio::MultiplexedConnection) -> u64 {
    let value: Option<u64> = connection
        .hget("rsmq:job:Q", "totalsent")
        .await
        .expect("read isolated queue counter");
    value.unwrap_or(0)
}

#[tokio::test]
#[ignore = "requires a dedicated empty Redis database; run explicitly with --ignored"]
async fn fifty_imported_revisions_preserve_current_page_and_send_no_jobs() {
    let redis_url = std::env::var("REDIS_URL").expect("dedicated REDIS_URL required");
    let client = redis::Client::open(redis_url).expect("valid dedicated Redis URL");
    let mut connection = client
        .get_multiplexed_async_connection()
        .await
        .expect("connect to dedicated Redis database");
    let mut queue = Rsmq::new_with_connection(connection.clone(), false, Some("rsmq"))
        .await
        .expect("initialize isolated queue client");
    queue
        .create_queue(
            JOB_QUEUE_NAME,
            JOB_QUEUE_PROCESS_TIME,
            JOB_QUEUE_DELAY,
            JOB_QUEUE_MAXIMUM_SIZE,
        )
        .await
        .expect("queue must be absent: use a fresh, dedicated Redis database");
    // Avoid seeding unrelated recurring jobs when the runner starts workers.
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let nav = run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id": site_id, "user_id": ADMIN_USER_ID,
            "slug": "history-queue-nav", "title": "Navigation",
            "wikitext": "Navigation source", "alt_title": null,
            "layout": "wikidot", "revision_comments": "Fixture",
            "bypass_filter": true, "ip_address": "127.0.0.1"
        })
    );
    site::ActiveModel {
        site_id: Set(site_id),
        top_bar_page: Set("history-queue-nav".into()),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .expect("configure a real site navigation fixture");
    let before = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": nav.page_id,
            "details": {"wikitext": true, "compiled": true}
        })
    )
    .unwrap();
    let sent_before = sent_count(&mut connection).await;
    let revisions: Vec<_> = (0..50)
        .map(|number| {
            json!({
                "source_revision_id": 1_700_000 + number,
                "source_revision_number": number,
                "source_author_id": 42,
                "source_created_at": "2021-04-29T12:00:00Z",
                "source_comments": "Archived metadata",
                "source_flags": if number == 10 { vec!["F"] } else { vec!["S"] },
                "source_title": null, "source_slug": null, "source_tags": null,
                "wikitext": format!("Historical body {number}"),
                "raw_source_html": format!("<div class=\"page-source\">Historical body {number}</div>"),
                "acquired_at": "2026-09-23T00:00:00Z",
                "representation": "display-decoded-not-byte-exact"
            })
        })
        .collect();
    let input = json!({
        "site_id": site_id, "page_id": nav.page_id,
        "source_page_id": 1_310_927_108,
        "expected_revision_id": before.revision_id,
        "revisions": revisions
    });
    assert_eq!(
        run_endpoint!(runner, import_wikidot_history, input.clone()).inserted,
        50
    );
    assert_eq!(sent_count(&mut connection).await, sent_before);
    assert_eq!(
        run_endpoint!(runner, import_wikidot_history, input).inserted,
        0
    );
    assert_eq!(sent_count(&mut connection).await, sent_before);
    let after = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": nav.page_id,
            "details": {"wikitext": true, "compiled": true}
        })
    )
    .unwrap();
    assert_eq!(after.revision_id, before.revision_id);
    assert_eq!(after.page_revision_count, before.page_revision_count);
    assert_eq!(after.wikitext, before.wikitext);
    assert_eq!(after.compiled_body_html, before.compiled_body_html);
}
