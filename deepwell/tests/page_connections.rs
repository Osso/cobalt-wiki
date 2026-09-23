//! Rerendering replaces a page's stored links even when many of them change at once.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::{page_connection, page_revision};
use deepwell::services::page::CreatePage;
use deepwell::services::page_revision::RerenderType;
use deepwell::services::{PageRevisionService, PageService, TextService};
use deepwell::types::{PageId, Reference, RerenderDepth};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set,
};
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
            revision_comments: "Link update fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("import fixture page")
    .page_id
}

fn links(targets: std::ops::Range<usize>) -> String {
    targets.map(|n| format!("[[[target-{n}]]]\n")).collect()
}

#[tokio::test]
async fn rerender_replaces_links_when_the_oldest_hundred_disappear() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    for n in 0..200 {
        import_page(&runner, site_id, &format!("target-{n}"), "Target").await;
    }
    let hub_id = import_page(&runner, site_id, "hub", &links(0..200)).await;
    let txn = runner.context().transaction();

    // The first hundred links are the oldest, so they fill the first chunk of
    // existing connections that the update walks.
    txn.execute_unprepared(&format!(
        "UPDATE page_connection SET created_at = created_at - interval '1 hour'
         WHERE from_page_id = {hub_id} AND to_page_id IN
           (SELECT page_id FROM page WHERE site_id = {site_id}
            AND slug IN ({}))",
        (0..100)
            .map(|n| format!("'target-{n}'"))
            .collect::<Vec<_>>()
            .join(", "),
    ))
    .await
    .unwrap();

    // Now the page only links to the newer hundred.
    let revision = PageRevisionService::get_latest(runner.context(), site_id, hub_id)
        .await
        .unwrap();
    let hash = TextService::create(runner.context(), links(100..200))
        .await
        .unwrap();
    page_revision::ActiveModel {
        revision_id: Set(revision.revision_id),
        wikitext_hash: Set(hash.to_vec()),
        ..Default::default()
    }
    .update(txn)
    .await
    .unwrap();
    let hub = PageService::get(runner.context(), site_id, Reference::Id(hub_id))
        .await
        .unwrap();
    PageRevisionService::rerender(
        runner.context(),
        PageId::from_page_model(&hub),
        RerenderDepth::default(),
        RerenderType::Full,
    )
    .await
    .expect("rerender with changed links");

    let connections = page_connection::Entity::find()
        .filter(page_connection::Column::FromPageId.eq(hub_id))
        .all(txn)
        .await
        .unwrap();
    assert_eq!(connections.len(), 100);
}
