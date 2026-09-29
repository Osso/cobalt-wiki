//! Page changes enqueue dependent rerenders, including category form-template edits.
//! Run with a dedicated Redis database: the assertions read queued job payloads.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::job::{
    JOB_QUEUE_DELAY, JOB_QUEUE_MAXIMUM_SIZE, JOB_QUEUE_NAME, JOB_QUEUE_PROCESS_TIME,
    JobService,
};
use deepwell::services::page::{CreatePage, EditPage, EditPageBody};
use deepwell::services::page_revision::RerenderType;
use deepwell::services::{PageRevisionService, PageService, RequestContext};
use deepwell::types::{Maybe, PageId, Reference};
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
    assert!(old_html.contains("Old specimen:"), "{old_html}");
    assert!(old_html.contains("Moth"), "{old_html}");
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

    // Retagging out of the selection also changes the listing. A worker has
    // started the first job; until then, the pending job would cover it.
    let roster_page = PageService::get(runner.context(), site_id, Reference::Id(roster))
        .await
        .unwrap();
    JobService::start_rerender_job(
        runner.context(),
        PageId::from_page_model(&roster_page),
        RerenderType::Standalone,
    )
    .await
    .unwrap();
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

#[tokio::test]
#[ignore = "requires a dedicated empty Redis database; run explicitly with --ignored"]
async fn site_tag_changes_rerender_every_listing_that_could_show_changed_pages() {
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
    let listing = |tag: &str| {
        format!(
            "[[module ListPages category=\"*\" tags=\"{tag}\"]]\n* %%title%%\n[[/module]]"
        )
    };

    // An edited page whose own listing shows another edited page must still be
    // rerendered, whichever of the two is edited first.
    let lister = create_page(
        &mut runner,
        site_id,
        "bulk-lister",
        &listing("bulk-new"),
        &["bulk-old"],
    )
    .await;
    create_page(&mut runner, site_id, "bulk-plain", "Plain", &["bulk-old"]).await;
    let shows_old =
        create_page(&mut runner, site_id, "shows-old", &listing("bulk-old"), &[]).await;
    let shows_new =
        create_page(&mut runner, site_id, "shows-new", &listing("bulk-new"), &[]).await;
    let shows_added = create_page(
        &mut runner,
        site_id,
        "shows-added",
        &listing("bulk-added"),
        &[],
    )
    .await;
    let unrelated = create_page(
        &mut runner,
        site_id,
        "shows-other",
        &listing("bulk-other"),
        &[],
    )
    .await;
    let queued = queued_rerenders(&mut connection).await;
    for page in [lister, shows_old, shows_new, shows_added, unrelated] {
        assert!(
            !queued.contains(&page),
            "fixture setup queued {page}: {queued:?}"
        );
    }

    runner.set_request_context(RequestContext {
        session: None,
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: None,
    });
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag": "bulk-old", "target_tag": "bulk-new"})
    );
    assert_eq!(preview.count, 2);
    run_endpoint!(
        runner,
        site_tag_apply,
        json!({
            "source_tag": "bulk-old", "target_tag": "bulk-new",
            "preview_token": preview.preview_token, "ip_address": common::IP_ADDRESS
        })
    );
    let queued = queued_rerenders(&mut connection).await;
    for page in [lister, shows_old, shows_new] {
        assert!(
            queued.contains(&page),
            "rename must queue {page}: {queued:?}"
        );
    }
    assert!(!queued.contains(&shows_added), "{queued:?}");
    assert!(!queued.contains(&unrelated), "{queued:?}");

    let selection = json!({
        "tag": "bulk-added", "existing_tag": "bulk-new", "category": null,
        "title_contains": null
    });
    let preview = run_endpoint!(runner, site_tag_add_preview, selection.clone());
    assert_eq!(preview.count, 2);
    let mut apply = selection;
    apply["preview_token"] = json!(preview.preview_token);
    apply["ip_address"] = json!(common::IP_ADDRESS);
    run_endpoint!(runner, site_tag_add_apply, apply);
    let queued = queued_rerenders(&mut connection).await;
    assert!(
        queued.contains(&shows_added),
        "add must queue its listing: {queued:?}"
    );
    assert!(!queued.contains(&unrelated), "{queued:?}");
}

/// `(page_id, type)` of every rerender job queued so far.
async fn queued_rerender_jobs(
    connection: &mut redis::aio::MultiplexedConnection,
) -> Vec<(i64, String)> {
    let messages: HashMap<String, Vec<u8>> =
        connection.hgetall("rsmq:job:Q").await.unwrap();
    messages
        .into_iter()
        .filter(|(field, _)| !field.contains(':') && field.len() > 10)
        .filter_map(|(_, body)| serde_json::from_slice::<serde_json::Value>(&body).ok())
        .filter_map(|job| {
            let id = job.pointer("/data/id/page_id")?.as_i64()?;
            let kind = job.pointer("/data/type")?.as_str()?.to_owned();
            Some((id, kind))
        })
        .collect()
}

#[tokio::test]
#[ignore = "requires a dedicated empty Redis database; run explicitly with --ignored"]
async fn site_tag_changes_queue_navigation_bars_instead_of_rendering_them() {
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

    // The top bar lists the destination tag, so the change alters every bar.
    let nav =
        PageService::get(runner.context(), site_id, Reference::Slug("nav:top".into()))
            .await
            .expect("seeded top bar");
    act_as_admin(&mut runner, site_id, Reference::Id(nav.page_id));
    PageService::edit(
        runner.context(),
        EditPage {
            site_id,
            page: Reference::Id(nav.page_id),
            last_revision_id: nav.latest_revision_id.unwrap(),
            revision_comments: "List destination tag".into(),
            user_id: ADMIN_USER_ID,
            body: EditPageBody {
                wikitext: Maybe::Set(
                    "[[module ListPages category=\"*\" tags=\"navbulk-new\"]]\n* %%title%%\n[[/module]]"
                        .into(),
                ),
                ..Default::default()
            },
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("edit top bar");
    let mut pages = Vec::new();
    for slug in ["navbulk-one", "navbulk-two"] {
        let page_id =
            create_page(&mut runner, site_id, slug, "Body", &["navbulk-old"]).await;
        let bar = PageRevisionService::get_latest(runner.context(), site_id, page_id)
            .await
            .unwrap()
            .compiled_top_bar_html_hash;
        assert!(bar.is_some(), "created page renders its top bar");
        pages.push((page_id, bar));
    }
    let queued = queued_rerender_jobs(&mut connection).await;
    for (page_id, _) in &pages {
        assert!(
            !queued.contains(&(*page_id, "nav".into())),
            "fixture setup queued {page_id}: {queued:?}"
        );
    }

    runner.set_request_context(RequestContext {
        session: None,
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: None,
    });
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag": "navbulk-old", "target_tag": "navbulk-new"})
    );
    assert_eq!(preview.count, 2);
    run_endpoint!(
        runner,
        site_tag_apply,
        json!({
            "source_tag": "navbulk-old", "target_tag": "navbulk-new",
            "preview_token": preview.preview_token, "ip_address": common::IP_ADDRESS
        })
    );

    let queued = queued_rerender_jobs(&mut connection).await;
    for (page_id, bar_before) in &pages {
        assert!(
            queued.contains(&(*page_id, "nav".into())),
            "bar rerender queued for {page_id}: {queued:?}"
        );
        let revision =
            PageRevisionService::get_latest(runner.context(), site_id, *page_id)
                .await
                .unwrap();
        assert_eq!(revision.tags, ["navbulk-new"]);
        assert_eq!(
            &revision.compiled_top_bar_html_hash, bar_before,
            "tag change keeps the previous bar until its rerender"
        );

        let page = PageService::get(runner.context(), site_id, Reference::Id(*page_id))
            .await
            .unwrap();
        PageRevisionService::rerender(
            runner.context(),
            PageId::from_page_model(&page),
            RerenderType::NavigationOnly,
        )
        .await
        .expect("run queued bar rerender");
        let rerendered =
            PageRevisionService::get_latest(runner.context(), site_id, *page_id)
                .await
                .unwrap()
                .compiled_top_bar_html_hash;
        assert!(rerendered.is_some());
        assert_ne!(
            &rerendered, bar_before,
            "rerendered bar lists the retagged pages"
        );
    }
}
