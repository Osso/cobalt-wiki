//! Native DB regression for exact-identity page imports and their first revision.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::page::{CreatePage, GetPageOutput};
use deepwell::services::{PageService, RequestContext};
use deepwell::types::{PageRevisionType, Reference};
use serde_json::json;

const SLUG: &str = "migration:chapter:part-3";
const NORMALIZED_SLUG: &str = "migration-chapter:part-3";
const TITLE: &str = "Chapter three — imported";
const SOURCE: &str = "+ Chapter three\r\n\r\n**Exact bytes:** café.\r\n";
const COMMENTS: &str = "Technical import fixture; not source authorship";

#[tokio::test]
async fn import_preserves_unmatched_closing_parentheses() {
    let (runner, site_id) = prepare_runner().await;
    let source = "A closing delimiter )) remains literal.\n";
    let mut input = import_input(site_id);
    input.wikitext = source.into();
    let imported = PageService::import(runner.context(), input)
        .await
        .expect("Unmatched closing parentheses must not panic during import");
    let stored = read_page(&runner, site_id, SLUG).await;
    assert_eq!(stored.page_id, imported.page_id);
    assert_eq!(stored.revision_number, 0);
    assert_eq!(stored.wikitext.as_deref(), Some(source));
    let rendered = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": SLUG, "details": {"compiled": true}})
    )
    .expect("Imported page must render");
    assert!(
        rendered
            .compiled_body_html
            .unwrap()
            .contains("delimiter )) remains literal.")
    );
}

async fn prepare_runner() -> (TestRunner, i64) {
    let mut runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .expect("Seeded site not found")
        .site
        .site_id;
    runner.set_request_context(RequestContext {
        session: None,
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug(SLUG.into())),
    });
    (runner, site_id)
}

fn import_input(site_id: i64) -> CreatePage {
    CreatePage {
        site_id,
        wikitext: SOURCE.into(),
        title: TITLE.into(),
        alt_title: None,
        slug: SLUG.into(),
        layout: None,
        revision_comments: COMMENTS.into(),
        tags: vec![],
        user_id: ADMIN_USER_ID,
        bypass_filter: true,
        ip_address: common::IP_ADDRESS,
    }
}

async fn read_page(runner: &TestRunner, site_id: i64, slug: &str) -> GetPageOutput {
    run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": slug, "details": {"wikitext": true}})
    )
    .expect("Page must be readable by its exact slug with a complete revision")
}

#[tokio::test]
async fn import_preserves_multicolon_identity_and_creates_initial_revision() {
    let (runner, site_id) = prepare_runner().await;
    let ordinary = PageService::create(runner.context(), import_input(site_id))
        .await
        .expect("Ordinary page creation failed");
    assert_eq!(ordinary.slug, NORMALIZED_SLUG);

    let imported = PageService::import(runner.context(), import_input(site_id))
        .await
        .expect("Exact-identity import failed");
    assert_eq!(imported.slug, SLUG);
    assert_ne!(imported.page_id, ordinary.page_id);
    assert_ne!(imported.revision_id, ordinary.revision_id);

    let stored = read_page(&runner, site_id, SLUG).await;
    assert_eq!(stored.page_id, imported.page_id);
    assert_eq!(stored.site_id, site_id);
    assert_eq!(stored.slug, SLUG);
    assert_eq!(stored.page_category_slug, "migration");
    assert_eq!(stored.revision_id, imported.revision_id);
    assert_eq!(stored.revision_number, 0);
    assert_eq!(stored.revision_type, PageRevisionType::Create);
    assert_eq!(stored.revision_user_id, ADMIN_USER_ID);
    assert_eq!(stored.revision_comments, COMMENTS);
    assert_eq!(stored.title, TITLE);
    assert_eq!(stored.wikitext.as_deref(), Some(SOURCE));
    assert!(stored.tags.is_empty());

    let normalized = read_page(&runner, site_id, NORMALIZED_SLUG).await;
    assert_eq!(normalized.page_id, ordinary.page_id);
    assert_eq!(normalized.revision_id, ordinary.revision_id);
    assert_eq!(normalized.page_category_slug, "migration-chapter");
    assert_eq!(normalized.slug, NORMALIZED_SLUG);
}

#[tokio::test]
async fn imported_page_retains_exact_tags_and_rejects_duplicate_identity() {
    let (runner, site_id) = prepare_runner().await;
    let imported = PageService::import(runner.context(), import_input(site_id))
        .await
        .expect("Exact-identity import failed");
    let tags = vec!["chapter", "non\u{a0}breaking"];
    let edited = run_endpoint!(
        runner,
        page_edit,
        json!({
            "site_id": site_id,
            "page": imported.page_id,
            "last_revision_id": imported.revision_id,
            "user_id": ADMIN_USER_ID,
            "tags": tags,
            "revision_comments": COMMENTS,
            "ip_address": common::IP_ADDRESS
        })
    )
    .expect("Tag update must create a revision");

    let mut conflicting = import_input(site_id);
    conflicting.title = "Must not overwrite existing page".into();
    conflicting.wikitext = "Must not replace imported source".into();
    PageService::import(runner.context(), conflicting)
        .await
        .expect_err("Duplicate exact identity must be rejected");

    let stored = read_page(&runner, site_id, SLUG).await;
    assert_eq!(stored.page_id, imported.page_id);
    assert_eq!(stored.revision_id, edited.revision_id);
    assert_eq!(stored.revision_number, 1);
    assert_eq!(stored.slug, SLUG);
    assert_eq!(stored.title, TITLE);
    assert_eq!(stored.wikitext.as_deref(), Some(SOURCE));
    assert_eq!(stored.tags, tags);
    assert_eq!(stored.revision_user_id, ADMIN_USER_ID);
}
