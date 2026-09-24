//! Page changes enqueue dependent rerenders, including category form-template edits.
//! Run with a dedicated Redis database: the assertions read queued job payloads.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::job::{
    JOB_QUEUE_DELAY, JOB_QUEUE_MAXIMUM_SIZE, JOB_QUEUE_NAME, JOB_QUEUE_PROCESS_TIME,
};
use deepwell::services::page::{CreatePage, EditPage, EditPageBody};
use deepwell::services::{PageRevisionService, PageService, RequestContext};
use deepwell::types::{Maybe, Reference};
use redis::AsyncCommands;
use rsmq_async::{Rsmq, RsmqConnection};
use serde_json::json;
use std::collections::HashMap;

fn act_as_admin(runner: &mut TestRunner, site_id: i64, page: Reference<'static>) {
    runner.set_request_context(RequestContext {
        session: None,
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(page),
    });
}

async fn create_page(
    runner: &mut TestRunner,
    site_id: i64,
    slug: &str,
    source: &str,
    tags: &[&str],
) -> i64 {
    act_as_admin(runner, site_id, Reference::Slug(slug.to_owned().into()));
    PageService::create(
        runner.context(),
        CreatePage {
            site_id,
            slug: slug.into(),
            title: slug.into(),
            wikitext: source.into(),
            alt_title: None,
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Listing invalidation fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("create fixture page")
    .page_id
}

/// Page IDs of every rerender job queued so far.
async fn queued_rerenders(
    connection: &mut redis::aio::MultiplexedConnection,
) -> Vec<i64> {
    let messages: HashMap<String, Vec<u8>> =
        connection.hgetall("rsmq:job:Q").await.unwrap();
    messages
        .into_iter()
        .filter(|(field, _)| !field.contains(':') && field.len() > 10)
        .filter_map(|(_, body)| serde_json::from_slice::<serde_json::Value>(&body).ok())
        .filter_map(|job| job.pointer("/data/id/page_id")?.as_i64())
        .collect()
}

#[tokio::test]
#[ignore = "requires a dedicated empty Redis database; run explicitly with --ignored"]
async fn editing_a_form_template_queues_only_its_category_without_changing_records() {
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
    let mut runner = TestRunner::setup_with_idle_job_workers().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let form = "====\n[[form]]\nfields:\n  name:\n    type: text\n[[/form]]\n";
    create_page(
        &mut runner,
        site_id,
        "specimen:_template",
        &format!("Old specimen: %%form_data{{name}}%%\n{form}"),
        &[],
    )
    .await;
    let record = "name: Moth\n";
    let dependent = create_page(&mut runner, site_id, "specimen:moth", record, &[]).await;
    let unrelated = create_page(
        &mut runner,
        site_id,
        "journal:entry",
        "Unrelated entry",
        &[],
    )
    .await;
    let before = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": "specimen:moth", "details": {"wikitext": true, "compiled": true}})
    )
    .unwrap();
    assert_eq!(before.wikitext.as_deref(), Some(record));
    let old_html = before.compiled_body_html.unwrap();
    assert!(old_html.contains("Old specimen: Moth"), "{old_html}");
    let original_revision =
        PageRevisionService::get_latest(runner.context(), site_id, dependent)
            .await
            .unwrap()
            .revision_id;
    let queued_before = queued_rerenders(&mut connection).await;

    let template_id = PageService::get(
        runner.context(),
        site_id,
        Reference::Slug("specimen:_template".into()),
    )
    .await
    .unwrap()
    .page_id;
    let template_revision =
        PageRevisionService::get_latest(runner.context(), site_id, template_id)
            .await
            .unwrap()
            .revision_id;
    act_as_admin(&mut runner, site_id, Reference::Id(template_id));
    PageService::edit(
        runner.context(),
        EditPage {
            site_id,
            page: Reference::Id(template_id),
            last_revision_id: template_revision,
            revision_comments: "Update form template".into(),
            user_id: ADMIN_USER_ID,
            body: EditPageBody {
                wikitext: Maybe::Set(format!(
                    "Updated specimen: %%form_data{{name}}%%\n{form}"
                )),
                ..Default::default()
            },
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("edit category form template");

    let queued_after = queued_rerenders(&mut connection).await;
    let count = |jobs: &[i64], id| jobs.iter().filter(|&&page_id| page_id == id).count();
    assert!(
        count(&queued_after, dependent) > count(&queued_before, dependent),
        "template edit must queue the dependent: {queued_after:?}"
    );
    assert_eq!(
        count(&queued_after, unrelated),
        count(&queued_before, unrelated),
        "template edit must not queue another category: {queued_after:?}"
    );
    let after = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": "specimen:moth", "details": {"wikitext": true, "compiled": true}})
    )
    .unwrap();
    assert_eq!(after.wikitext.as_deref(), Some(record));
    assert_eq!(after.compiled_body_html.as_deref(), Some(old_html.as_str()));
    let current_revision =
        PageRevisionService::get_latest(runner.context(), site_id, dependent)
            .await
            .unwrap()
            .revision_id;
    assert_eq!(current_revision, original_revision);
}

#[tokio::test]
#[ignore = "requires a dedicated empty Redis database; run explicitly with --ignored"]
async fn page_changes_rerender_listings_that_could_show_them() {
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
    let mut runner = TestRunner::setup_with_idle_job_workers().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let roster = create_page(
        &mut runner,
        site_id,
        "roster",
        "[[module ListPages category=\"character\" tags=\"_completed\"]]\n* %%title%%\n[[/module]]",
        &[],
    )
    .await;

    let tag_cloud = create_page(
        &mut runner,
        site_id,
        "tag-cloud",
        "[[module TagCloud]]",
        &[],
    )
    .await;

    create_page(
        &mut runner,
        site_id,
        "writing:unrelated",
        "Story",
        &["_completed"],
    )
    .await;
    let queued = queued_rerenders(&mut connection).await;
    assert!(!queued.contains(&roster), "{queued:?}");
    // The tag cloud counts every page's tags.
    assert!(queued.contains(&tag_cloud), "{queued:?}");

    let alpha = create_page(
        &mut runner,
        site_id,
        "character:alpha",
        "Alpha",
        &["_completed"],
    )
    .await;
    let queued = queued_rerenders(&mut connection).await;
    assert_eq!(
        queued.iter().filter(|&&id| id == roster).count(),
        1,
        "{queued:?}"
    );

    // Retagging out of the selection also changes the listing.
    let last_revision_id =
        PageRevisionService::get_latest(runner.context(), site_id, alpha)
            .await
            .unwrap()
            .revision_id;
    act_as_admin(&mut runner, site_id, Reference::Id(alpha));
    PageService::edit(
        runner.context(),
        EditPage {
            site_id,
            page: Reference::Id(alpha),
            last_revision_id,
            revision_comments: "Retag".into(),
            user_id: ADMIN_USER_ID,
            body: EditPageBody {
                tags: Maybe::Set(vec!["draft".into()]),
                ..Default::default()
            },
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("retag page");
    let queued = queued_rerenders(&mut connection).await;
    assert_eq!(
        queued.iter().filter(|&&id| id == roster).count(),
        2,
        "{queued:?}"
    );
}
