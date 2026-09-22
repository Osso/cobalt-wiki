//! Navigation invalidation must be finite and limited to configured nav pages.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::config::{Config, Secrets};
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::{page, site};
use deepwell::services::PageService;
use deepwell::services::job::{JOB_QUEUE_NAME, Job};
use deepwell::services::page::CreatePage;
use deepwell::services::page_revision::{PageRevisionService, RerenderType};
use deepwell::types::{PageId, RerenderDepth};
use redis::aio::MultiplexedConnection;
use rsmq_async::{Rsmq, RsmqConnection};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct PrivateValkey {
    child: Child,
    socket: PathBuf,
}

impl PrivateValkey {
    async fn start() -> Self {
        let socket = std::env::temp_dir().join(format!(
            "nav-invalidation-{}-{}.sock",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let mut child = Command::new("valkey-server")
            .args([
                "--port",
                "0",
                "--save",
                "",
                "--appendonly",
                "no",
                "--unixsocket",
            ])
            .arg(&socket)
            .args(["--loglevel", "warning"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("local valkey-server must be available");
        for _ in 0..100 {
            if socket.exists() {
                return Self { child, socket };
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let _ = child.kill();
        child.wait().expect("reap failed private Valkey startup");
        panic!("private Valkey socket did not become ready");
    }

    fn url(&self) -> String {
        format!("redis+unix://{}", self.socket.display())
    }
}

impl Drop for PrivateValkey {
    fn drop(&mut self) {
        self.child.kill().expect("kill private Valkey");
        self.child.wait().expect("reap private Valkey");
        let _ = std::fs::remove_file(&self.socket);
    }
}

async fn private_queue(url: &str) -> Rsmq {
    let client = redis::Client::open(url).expect("private Redis URL");
    let connection: MultiplexedConnection = client
        .get_multiplexed_async_connection()
        .await
        .expect("private Redis connection");
    let mut queue = Rsmq::new_with_connection(connection, false, Some("rsmq"))
        .await
        .expect("private queue connection");
    queue
        .create_queue(
            JOB_QUEUE_NAME,
            Some(Duration::from_secs(30)),
            None,
            Some(1024),
        )
        .await
        .expect("create isolated job queue");
    queue
}

async fn take_jobs(queue: &mut Rsmq) -> Vec<Job> {
    let mut jobs = Vec::new();
    while let Some(message) = queue
        .receive_message::<Vec<u8>>(JOB_QUEUE_NAME, None)
        .await
        .expect("read isolated queue")
    {
        jobs.push(serde_json::from_slice(&message.message).expect("valid queued job"));
        queue
            .delete_message(JOB_QUEUE_NAME, &message.id)
            .await
            .expect("delete only this test's private message");
    }
    jobs
}

async fn import_fixture(runner: &TestRunner, site_id: i64, slug: &str) -> PageId {
    let output = PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            slug: slug.into(),
            title: slug.into(),
            wikitext: format!("Fixture {slug}"),
            alt_title: None,
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Nav invalidation fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("import fixture page");
    let page = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": output.page_id})
    )
    .unwrap();
    PageId {
        site_id,
        category_id: page.page_category_id,
        page_id: page.page_id,
    }
}

#[tokio::test]
async fn configured_nav_rerender_fans_out_once_but_other_pages_and_nav_only_do_not() {
    let private = PrivateValkey::start().await;
    let redis_url = private.url();
    let mut queue = private_queue(&redis_url).await;
    let mut secrets = Secrets::load();
    secrets.redis_url = redis_url;
    let mut config = Config::integration_testing();
    // The real worker is started by the harness; keep it idle while fixture
    // database rows live in an uncommitted rollback transaction.
    config.job_min_poll_delay = Duration::from_secs(60);
    config.job_max_poll_delay = Duration::from_secs(60);
    config.rerender_skip.clear();
    let runner = TestRunner::setup_with_config(config, secrets).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(take_jobs(&mut queue).await.is_empty());

    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let nav = import_fixture(&runner, site_id, "nav-invalidation-nav").await;
    let first = import_fixture(&runner, site_id, "nav-invalidation-first").await;
    let second = import_fixture(&runner, site_id, "nav-invalidation-second").await;
    site::ActiveModel {
        site_id: Set(site_id),
        top_bar_page: Set("nav-invalidation-nav".into()),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .expect("configure site nav fixture");
    take_jobs(&mut queue).await;

    PageRevisionService::rerender(
        runner.context(),
        first,
        RerenderDepth::default(),
        RerenderType::Full,
    )
    .await
    .expect("ordinary full rerender");
    assert!(
        take_jobs(&mut queue).await.is_empty(),
        "ordinary page must not fan out navigation work"
    );

    PageRevisionService::rerender(
        runner.context(),
        nav,
        RerenderDepth::default(),
        RerenderType::Full,
    )
    .await
    .expect("configured nav full rerender");
    let jobs = take_jobs(&mut queue).await;
    let nav_ids: HashSet<_> = jobs
        .iter()
        .filter_map(|job| match job {
            Job::RerenderPage {
                id,
                r#type: RerenderType::NavigationOnly,
                ..
            } => Some(id.page_id),
            _ => None,
        })
        .collect();
    let site_pages: HashSet<_> = page::Entity::find()
        .filter(page::Column::SiteId.eq(site_id))
        .filter(page::Column::DeletedAt.is_null())
        .all(runner.context().transaction())
        .await
        .expect("read site fixture pages")
        .into_iter()
        .map(|page| page.page_id)
        .collect();
    assert!(site_pages.is_superset(&HashSet::from([
        nav.page_id,
        first.page_id,
        second.page_id
    ])));
    assert_eq!(
        nav_ids.len(),
        jobs.len(),
        "one nav-only job per affected page: {jobs:?}"
    );
    assert_eq!(nav_ids, site_pages, "all site pages receive nav-only jobs");

    for job in jobs {
        let Job::RerenderPage {
            id,
            depth,
            r#type: RerenderType::NavigationOnly,
        } = job
        else {
            panic!("unexpected nav fanout job: {job:?}");
        };
        PageRevisionService::rerender(
            runner.context(),
            id,
            depth,
            RerenderType::NavigationOnly,
        )
        .await
        .expect("process nav-only rerender");
    }
    assert!(
        take_jobs(&mut queue).await.is_empty(),
        "nav-only rerender must not produce more site-wide work"
    );
}
