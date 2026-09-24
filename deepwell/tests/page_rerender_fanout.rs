//! Rerenders must not multiply into dependent jobs.
//!
//! Fixture: `nav:fanout-top` is the site top bar and includes `component:box`, which
//! includes `component:inner`; `writing:a` and `writing:b` include `component:box`;
//! `roster` lists the `writing` category, which has a `_template`; `tag-cloud`
//! lists every page.
//!
//! Run with a dedicated Redis database: each test flushes it and reads queued job payloads.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::config::Config;
use deepwell::constants::{ADMIN_USER_ID, SYSTEM_USER_ID};
use deepwell::models::page_revision;
use deepwell::services::job::{
    JOB_QUEUE_DELAY, JOB_QUEUE_MAXIMUM_SIZE, JOB_QUEUE_NAME, JOB_QUEUE_PROCESS_TIME,
    JobService,
};
use deepwell::services::page::{CreatePage, EditPage, EditPageBody};
use deepwell::services::page_revision::RerenderType;
use deepwell::services::{PageRevisionService, PageService, RequestContext, TextService};
use deepwell::types::{Maybe, PageId, PageOrder, Reference, RerenderDepth};
use redis::AsyncCommands;
use redis::aio::MultiplexedConnection;
use rsmq_async::{Rsmq, RsmqConnection};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// One queued `rerender_page` job.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct QueuedRerender {
    page_id: i64,
    kind: String,
    depth: u32,
}

impl QueuedRerender {
    fn parse(body: &[u8]) -> Option<(Self, PageId, RerenderType)> {
        let job: Value = serde_json::from_slice(body).ok()?;
        let data = job.get("data")?;
        let id: PageId = serde_json::from_value(data.get("id")?.clone()).ok()?;
        let rerender_type: RerenderType =
            serde_json::from_value(data.get("type")?.clone()).ok()?;
        let queued = QueuedRerender {
            page_id: id.page_id,
            kind: data.get("type")?.as_str()?.to_owned(),
            depth: u32::try_from(data.get("depth")?.as_u64()?).ok()?,
        };
        Some((queued, id, rerender_type))
    }
}

/// Production's `rerender-skip` rules (`config.example.toml`).
fn production_like_config() -> Config {
    let mut config = Config::integration_testing();
    config.job_min_poll_delay = std::time::Duration::from_secs(3600);
    config.job_max_poll_delay = std::time::Duration::from_secs(3600);
    config.rerender_skip = vec![
        (3, Some(time::Duration::milliseconds(100))),
        (10, Some(time::Duration::milliseconds(1500))),
        (50, None),
    ];
    config
}

async fn fresh_queue() -> (MultiplexedConnection, Rsmq) {
    let client = redis::Client::open(std::env::var("REDIS_URL").unwrap()).unwrap();
    let mut connection = client.get_multiplexed_async_connection().await.unwrap();
    let _: () = redis::cmd("FLUSHDB")
        .query_async(&mut connection)
        .await
        .expect("flush the dedicated Redis database");
    let mut rsmq = Rsmq::new_with_connection(connection.clone(), false, Some("rsmq"))
        .await
        .unwrap();
    rsmq.create_queue(
        JOB_QUEUE_NAME,
        JOB_QUEUE_PROCESS_TIME,
        JOB_QUEUE_DELAY,
        JOB_QUEUE_MAXIMUM_SIZE,
    )
    .await
    .unwrap();
    (connection, rsmq)
}

async fn sent_count(connection: &mut MultiplexedConnection) -> u64 {
    let count: Option<u64> = connection.hget("rsmq:job:Q", "totalsent").await.unwrap();
    count.unwrap_or(0)
}

/// Every rerender job currently in the queue.
async fn queued(connection: &mut MultiplexedConnection) -> Vec<QueuedRerender> {
    let messages: HashMap<String, Vec<u8>> =
        connection.hgetall("rsmq:job:Q").await.unwrap();
    let mut jobs: Vec<_> = messages
        .into_iter()
        .filter(|(field, _)| !field.contains(':') && field.len() > 10)
        .filter_map(|(_, body)| QueuedRerender::parse(&body))
        .map(|(queued, _, _)| queued)
        .collect();
    jobs.sort();
    jobs
}

/// Number of queued jobs per `(page_id, kind)`.
fn per_page(jobs: &[QueuedRerender]) -> BTreeMap<(i64, String), usize> {
    let mut counts = BTreeMap::new();
    for job in jobs {
        *counts.entry((job.page_id, job.kind.clone())).or_default() += 1;
    }
    counts
}

/// Runs queued rerender jobs the way the job worker does, until the queue is
/// empty. Returns every job processed, in order.
async fn drain(runner: &TestRunner, rsmq: &mut Rsmq) -> Vec<QueuedRerender> {
    let mut processed = Vec::new();
    while let Some(message) = rsmq
        .receive_message::<Vec<u8>>(JOB_QUEUE_NAME, None)
        .await
        .unwrap()
    {
        let (job, id, rerender_type) =
            QueuedRerender::parse(&message.message).expect("rerender job");
        JobService::start_rerender_job(runner.context(), id, rerender_type)
            .await
            .unwrap();
        PageRevisionService::rerender(
            runner.context(),
            id,
            RerenderDepth(job.depth),
            rerender_type,
        )
        .await
        .unwrap();
        rsmq.delete_message(JOB_QUEUE_NAME, &message.id)
            .await
            .unwrap();
        processed.push(job);
        assert!(processed.len() < 100_000, "rerender jobs never drain");
    }
    processed
}

fn act_as_admin(runner: &mut TestRunner, site_id: i64, page: Reference<'static>) {
    runner.set_request_context(RequestContext {
        session: None,
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(page),
    });
}

async fn create_page(runner: &mut TestRunner, site_id: i64, slug: &str, source: &str) {
    act_as_admin(runner, site_id, Reference::Slug(slug.to_owned().into()));
    PageService::create(
        runner.context(),
        CreatePage {
            site_id,
            slug: slug.into(),
            title: slug.into(),
            wikitext: source.into(),
            alt_title: None,
            tags: vec![],
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Rerender fan-out fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("create fixture page");
}

async fn edit_source(runner: &mut TestRunner, site_id: i64, slug: &str, source: &str) {
    let page = PageService::get(
        runner.context(),
        site_id,
        Reference::Slug(slug.to_owned().into()),
    )
    .await
    .unwrap();
    let last_revision_id =
        PageRevisionService::get_latest(runner.context(), site_id, page.page_id)
            .await
            .unwrap()
            .revision_id;
    act_as_admin(runner, site_id, Reference::Id(page.page_id));
    PageService::edit(
        runner.context(),
        EditPage {
            site_id,
            page: Reference::Id(page.page_id),
            last_revision_id,
            revision_comments: "Change source".into(),
            user_id: ADMIN_USER_ID,
            body: EditPageBody {
                wikitext: Maybe::Set(source.into()),
                ..Default::default()
            },
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("edit page source");
}

struct Fixture {
    site_id: i64,
    ids: HashMap<&'static str, i64>,
    /// Every live page on the site: the consumers of the site top bar.
    live_pages: Vec<i64>,
}

impl Fixture {
    fn id(&self, slug: &str) -> i64 {
        self.ids[slug]
    }
}

async fn setup_fixture(runner: &mut TestRunner) -> Fixture {
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    run_endpoint!(
        runner,
        site_update,
        json!({
            "site": site_id,
            "user_id": SYSTEM_USER_ID,
            "top_bar_page": "nav:fanout-top",
            "side_bar_page": "",
            "ip_address": "127.0.0.1",
        }),
    );
    let pages = [
        ("component:inner", "Inner v1"),
        ("component:box", "[[include component:inner]]\nBox"),
        ("nav:fanout-top", "[[include component:box]]"),
        ("writing:a", "[[include component:box]]\nStory A"),
        ("writing:b", "[[include component:box]]\nStory B"),
        ("writing:_template", "%%content%%\n\nWriting footer"),
        ("tag-cloud", "[[module TagCloud]]"),
        (
            "roster",
            "[[module ListPages category=\"writing\"]]\n* %%title%%\n[[/module]]",
        ),
    ];
    let mut ids = HashMap::new();
    for (slug, source) in pages {
        create_page(runner, site_id, slug, source).await;
        let page =
            PageService::get(runner.context(), site_id, Reference::Slug(slug.into()))
                .await
                .unwrap();
        ids.insert(slug, page.page_id);
    }
    let live_pages = PageService::get_all(
        runner.context(),
        site_id,
        None,
        Some(false),
        PageOrder::default(),
    )
    .await
    .unwrap()
    .into_iter()
    .map(|page| page.page_id)
    .collect();
    Fixture {
        site_id,
        ids,
        live_pages,
    }
}

/// Marks every page's stored render as coming from an older renderer build.
async fn age_renderer_build(runner: &TestRunner, fixture: &Fixture) {
    for &page_id in &fixture.live_pages {
        let revision =
            PageRevisionService::get_latest(runner.context(), fixture.site_id, page_id)
                .await
                .unwrap();
        page_revision::ActiveModel {
            revision_id: Set(revision.revision_id),
            compiled_generator: Set("older-renderer-build".into()),
            ..Default::default()
        }
        .update(runner.context().transaction())
        .await
        .unwrap();
    }
}

#[tokio::test]
#[ignore = "requires a dedicated Redis database; run explicitly with --ignored"]
async fn renderer_version_sweep_queues_no_dependents() {
    let mut runner = TestRunner::setup_with_config(production_like_config()).await;
    let fixture = setup_fixture(&mut runner).await;
    let (mut connection, mut rsmq) = fresh_queue().await;
    age_renderer_build(&runner, &fixture).await;

    // The post-deploy sweep: a default ("full") page_rerender of every page.
    for &page_id in &fixture.live_pages {
        let page =
            PageService::get(runner.context(), fixture.site_id, Reference::Id(page_id))
                .await
                .unwrap();
        run_endpoint!(
            runner,
            page_rerender,
            json!({
                "site_id": fixture.site_id,
                "category_id": page.page_category_id,
                "page_id": page_id,
            })
        );
    }
    let after_sweep = queued(&mut connection).await;
    let drained = drain(&runner, &mut rsmq).await;
    eprintln!(
        "sweep of {} pages: {} jobs queued by the sweep, {} jobs run in total, by depth {:?}",
        fixture.live_pages.len(),
        after_sweep.len(),
        drained.len(),
        depth_histogram(&drained),
    );
    assert_eq!(after_sweep, vec![], "sweep queued dependents");
    assert_eq!(drained, vec![], "sweep jobs queued further jobs");
}

fn depth_histogram(jobs: &[QueuedRerender]) -> BTreeMap<(u32, String), usize> {
    let mut counts = BTreeMap::new();
    for job in jobs {
        *counts.entry((job.depth, job.kind.clone())).or_default() += 1;
    }
    counts
}

/// Checks the jobs one change to `component:inner` must produce: one
/// rerender of each page that includes it or lists every page, and one
/// navigation rerender of every page, because the site top bar includes it.
///
/// Pages outside the fixture (the seeded site's own listings) may also be
/// queued, but no job may be queued twice.
fn assert_inner_change_jobs(jobs: &[QueuedRerender], fixture: &Fixture) {
    let counts = per_page(jobs);
    let duplicates: Vec<_> = counts.iter().filter(|&(_, &count)| count > 1).collect();
    assert!(duplicates.is_empty(), "duplicate jobs: {duplicates:?}");

    let fixture_ids: BTreeSet<i64> = fixture.ids.values().copied().collect();
    let rerendered: BTreeSet<i64> = counts
        .keys()
        .filter(|(page_id, kind)| kind == "standalone" && fixture_ids.contains(page_id))
        .map(|&(page_id, _)| page_id)
        .collect();
    let expected: BTreeSet<i64> = [
        "component:box",
        "nav:fanout-top",
        "writing:a",
        "writing:b",
        "tag-cloud",
    ]
    .into_iter()
    .map(|slug| fixture.id(slug))
    .collect();
    assert_eq!(rerendered, expected, "rerendered fixture pages");

    let nav: BTreeSet<i64> = counts
        .keys()
        .filter(|(_, kind)| kind == "nav")
        .map(|&(page_id, _)| page_id)
        .collect();
    let live: BTreeSet<i64> = fixture.live_pages.iter().copied().collect();
    assert_eq!(nav, live, "navigation rerenders");
    assert!(
        counts
            .keys()
            .all(|(_, kind)| kind == "standalone" || kind == "nav"),
        "{counts:?}"
    );
}

#[tokio::test]
#[ignore = "requires a dedicated Redis database; run explicitly with --ignored"]
async fn source_change_queues_each_dependent_once_and_jobs_queue_nothing() {
    let mut runner = TestRunner::setup_with_config(production_like_config()).await;
    let fixture = setup_fixture(&mut runner).await;
    let (mut connection, mut rsmq) = fresh_queue().await;

    edit_source(&mut runner, fixture.site_id, "component:inner", "Inner v2").await;
    let after_edit = queued(&mut connection).await;
    let sent_after_edit = sent_count(&mut connection).await;
    let drained = drain(&runner, &mut rsmq).await;
    eprintln!(
        "edit of component:inner: {} jobs queued by the edit, {} jobs run in total, by depth {:?}",
        after_edit.len(),
        drained.len(),
        depth_histogram(&drained),
    );
    assert_inner_change_jobs(&after_edit, &fixture);
    assert_eq!(
        sent_count(&mut connection).await,
        sent_after_edit,
        "dependent jobs queued further jobs"
    );

    // Every dependent now shows the change, including the navigation bars.
    for slug in ["component:box", "nav:fanout-top", "writing:a", "writing:b"] {
        let page = run_endpoint!(
            runner,
            page_get,
            json!({"site_id": fixture.site_id, "page": slug, "details": {"compiled_html": true}})
        )
        .unwrap();
        let html = page.compiled_body_html.unwrap();
        assert!(html.contains("Inner v2"), "{slug}: {html}");
    }
    let roster = PageRevisionService::get_latest(
        runner.context(),
        fixture.site_id,
        fixture.id("roster"),
    )
    .await
    .unwrap();
    let top_bar = TextService::get(
        runner.context(),
        roster
            .compiled_top_bar_html_hash
            .as_deref()
            .expect("top bar"),
    )
    .await
    .unwrap();
    assert!(
        top_bar.contains("Inner v2"),
        "top bar of an unrelated page: {top_bar}"
    );
}

#[tokio::test]
#[ignore = "requires a dedicated Redis database; run explicitly with --ignored"]
async fn pending_rerenders_collapse_until_a_worker_starts_them() {
    let mut runner = TestRunner::setup_with_config(production_like_config()).await;
    let fixture = setup_fixture(&mut runner).await;
    let (mut connection, mut rsmq) = fresh_queue().await;

    edit_source(&mut runner, fixture.site_id, "component:inner", "Inner v2").await;
    edit_source(&mut runner, fixture.site_id, "component:inner", "Inner v3").await;
    // The second change, while its jobs are pending, queues nothing more.
    assert_inner_change_jobs(&queued(&mut connection).await, &fixture);

    // Once the worker has started them, a new change queues them again.
    drain(&runner, &mut rsmq).await;
    edit_source(&mut runner, fixture.site_id, "component:inner", "Inner v4").await;
    assert_inner_change_jobs(&queued(&mut connection).await, &fixture);
}
