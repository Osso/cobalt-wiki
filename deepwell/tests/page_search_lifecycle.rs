//! Page lifecycle search updates stay pending after each service transaction.
//! The test runner rolls back its outer transaction, so nested commits remain isolated.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::search_index_pending;
use deepwell::services::page::CreatePage;
use deepwell::services::page_revision::RerenderType;
use deepwell::services::{
    PageRevisionService, PageService, RequestContext, ServiceContext,
};
use deepwell::types::{PageId, Reference};
use sea_orm::{EntityTrait, TransactionTrait};
use serde_json::json;

async fn pending_generation(runner: &TestRunner, page_id: i64) -> Option<i64> {
    search_index_pending::Entity::find_by_id(page_id)
        .one(runner.context().transaction())
        .await
        .expect("read search outbox in fixture transaction")
        .map(|pending| pending.generation)
}

fn page_request(site_id: i64, slug: &str) -> RequestContext {
    RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug(slug.to_owned().into())),
        ..Default::default()
    }
}

async fn setup() -> (TestRunner, i64) {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .expect("seeded site")
        .site
        .site_id;
    (runner, site_id)
}

#[tokio::test]
async fn create_edit_delete_restore_and_rerender_advance_pending_generation() {
    let (runner, site_id) = setup().await;
    let slug = format!("search-lifecycle-{}", uuid::Uuid::new_v4().simple());

    let step = runner.context().transaction().begin().await.unwrap();
    let ctx = ServiceContext::new(runner.state(), &step)
        .with_request(page_request(site_id, &slug));
    let created = PageService::create(
        &ctx,
        CreatePage {
            site_id,
            slug: slug.clone(),
            title: "Search lifecycle".into(),
            wikitext: "First body".into(),
            alt_title: None,
            layout: None,
            revision_comments: "Create".into(),
            tags: vec!["first".into()],
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("create page");
    step.commit().await.unwrap();
    let mut generation = pending_generation(&runner, created.page_id)
        .await
        .expect("create must enqueue pending page");

    let step = runner.context().transaction().begin().await.unwrap();
    let ctx = ServiceContext::new(runner.state(), &step)
        .with_request(page_request(site_id, &slug));
    let edited = deepwell::endpoints::all::page_edit(
        &ctx,
        common::make_params(json!({
            "site_id": site_id,
            "page": created.page_id,
            "last_revision_id": created.revision_id,
            "title": "Edited search title",
            "revision_comments": "Edit",
            "user_id": ADMIN_USER_ID,
            "ip_address": common::IP_ADDRESS
        })),
    )
    .await
    .expect("edit page")
    .expect("edit must create revision");
    step.commit().await.unwrap();
    let next = pending_generation(&runner, created.page_id)
        .await
        .expect("edit must leave page pending");
    assert!(next > generation, "edit must advance pending generation");
    generation = next;

    let step = runner.context().transaction().begin().await.unwrap();
    let ctx = ServiceContext::new(runner.state(), &step)
        .with_request(page_request(site_id, &slug));
    deepwell::endpoints::all::page_delete(
        &ctx,
        common::make_params(json!({
            "site_id": site_id,
            "page": created.page_id,
            "last_revision_id": edited.revision_id,
            "revision_comments": "Delete",
            "user_id": ADMIN_USER_ID,
            "ip_address": common::IP_ADDRESS
        })),
    )
    .await
    .expect("delete page");
    step.commit().await.unwrap();
    let next = pending_generation(&runner, created.page_id)
        .await
        .expect("deleted page must remain pending for index removal");
    assert!(next > generation, "delete must advance pending generation");
    generation = next;

    let step = runner.context().transaction().begin().await.unwrap();
    let ctx = ServiceContext::new(runner.state(), &step)
        .with_request(page_request(site_id, &slug));
    deepwell::endpoints::all::page_restore(
        &ctx,
        common::make_params(json!({
            "site_id": site_id,
            "page_id": created.page_id,
            "revision_comments": "Restore",
            "user_id": ADMIN_USER_ID,
            "ip_address": common::IP_ADDRESS
        })),
    )
    .await
    .expect("restore page");
    step.commit().await.unwrap();
    let next = pending_generation(&runner, created.page_id)
        .await
        .expect("restored page must remain pending");
    assert!(next > generation, "restore must advance pending generation");
    generation = next;

    let page =
        PageService::get(runner.context(), site_id, Reference::Id(created.page_id))
            .await
            .expect("restored page exists");
    let step = runner.context().transaction().begin().await.unwrap();
    let ctx = ServiceContext::new(runner.state(), &step)
        .with_request(page_request(site_id, &slug));
    PageRevisionService::rerender(
        &ctx,
        PageId::from_page_model(&page),
        RerenderType::Standalone,
    )
    .await
    .expect("rerender page");
    step.commit().await.unwrap();
    let next = pending_generation(&runner, created.page_id)
        .await
        .expect("rerendered page must remain pending");
    assert!(
        next > generation,
        "rerender must advance pending generation"
    );
}

#[tokio::test]
async fn import_enqueues_pending_search_update() {
    let (runner, site_id) = setup().await;
    let slug = format!("search-import-{}", uuid::Uuid::new_v4().simple());
    let step = runner.context().transaction().begin().await.unwrap();
    let ctx = ServiceContext::new(runner.state(), &step)
        .with_request(page_request(site_id, &slug));
    let imported = PageService::import(
        &ctx,
        CreatePage {
            site_id,
            slug,
            title: "Imported title".into(),
            wikitext: "Imported body".into(),
            alt_title: None,
            layout: None,
            revision_comments: "Import".into(),
            tags: vec!["imported".into()],
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("import page");
    step.commit().await.unwrap();
    assert!(
        pending_generation(&runner, imported.page_id)
            .await
            .is_some(),
        "import must enqueue pending page"
    );
}
