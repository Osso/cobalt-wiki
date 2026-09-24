//! Search validates Meili candidates against current committed-in-fixture page state.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::page_revision;
use deepwell::services::page::CreatePage;
use deepwell::services::page_revision::RerenderType;
use deepwell::services::search::{SearchDocument, SearchRequest, SearchService};
use deepwell::services::{PageRevisionService, PageService, RequestContext, TextService};
use deepwell::types::{PageId, Reference};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn import_page(
    runner: &TestRunner,
    site_id: i64,
    slug: &str,
    body: &str,
) -> SearchDocument {
    let created = PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            slug: slug.into(),
            title: slug.into(),
            wikitext: body.into(),
            alt_title: None,
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Search freshness fixture".into(),
            tags: vec![],
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("import fixture page");
    let revision =
        PageRevisionService::get_latest(runner.context(), site_id, created.page_id)
            .await
            .expect("read imported revision");
    let html = TextService::get(runner.context(), &revision.compiled_body_html_hash)
        .await
        .expect("read compiled body");
    let body = scraper::Html::parse_fragment(&html)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    SearchDocument {
        page_id: created.page_id,
        site_id,
        revision_id: created.revision_id,
        title: revision.title,
        slug: slug.into(),
        tags: revision.tags,
        body,
    }
}

async fn serve_hits(listener: TcpListener, documents: Vec<SearchDocument>) {
    let response = json!({ "hits": documents }).to_string();
    for _ in 0..2 {
        let (mut socket, _) = listener.accept().await.expect("accept search request");
        let mut request = [0; 8192];
        let size = socket
            .read(&mut request)
            .await
            .expect("read search request");
        assert!(
            String::from_utf8_lossy(&request[..size])
                .starts_with("POST /indexes/pages/search "),
            "only search requests expected"
        );
        let reply = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.len(),
            response
        );
        socket
            .write_all(reply.as_bytes())
            .await
            .expect("reply with hits");
    }
}

#[tokio::test]
async fn same_revision_rerender_filters_stale_hit_before_visible_pagination() {
    let mut runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .expect("seeded site")
        .site
        .site_id;
    let prefix = format!("freshness-{}", uuid::Uuid::new_v4().simple());
    let stale =
        import_page(&runner, site_id, &format!("{prefix}-stale"), "Old body").await;
    let first = import_page(
        &runner,
        site_id,
        &format!("{prefix}-first"),
        "First fresh body",
    )
    .await;
    let second = import_page(
        &runner,
        site_id,
        &format!("{prefix}-second"),
        "Second fresh body",
    )
    .await;
    assert_eq!(stale.body, "Old body");
    assert_eq!(first.body, "First fresh body");
    assert_eq!(second.body, "Second fresh body");

    // Simulate a renderer/source update without creating a new revision, then rerender through service.
    let replacement_hash = TextService::create(runner.context(), "New body".into())
        .await
        .expect("store replacement source");
    page_revision::ActiveModel {
        revision_id: Set(stale.revision_id),
        wikitext_hash: Set(replacement_hash.to_vec()),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .expect("replace source on same revision");
    let page = PageService::get(runner.context(), site_id, Reference::Id(stale.page_id))
        .await
        .expect("read rerender target");
    PageRevisionService::rerender(
        runner.context(),
        PageId::from_page_model(&page),
        RerenderType::Standalone,
    )
    .await
    .expect("rerender existing revision");
    let current =
        PageRevisionService::get_latest(runner.context(), site_id, stale.page_id)
            .await
            .expect("read rerendered revision");
    assert_eq!(current.revision_id, stale.revision_id);
    let current_html =
        TextService::get(runner.context(), &current.compiled_body_html_hash)
            .await
            .expect("read rerendered body");
    assert!(current_html.contains("New body"));
    assert!(!current_html.contains("Old body"));

    runner.set_request_context(RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake Meili");
    let url = format!("http://{}", listener.local_addr().unwrap());
    let responder = tokio::spawn(serve_hits(
        listener,
        vec![stale, first.clone(), second.clone()],
    ));
    let search = SearchService::new(url, "test-key".into());

    for (offset, expected, has_more) in [(0, &first, true), (1, &second, false)] {
        let result = search
            .page_search(
                runner.context(),
                SearchRequest {
                    query: "body".into(),
                    offset,
                    limit: 1,
                },
            )
            .await
            .expect("search current pages");
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].page_id, expected.page_id);
        assert_eq!(result.hits[0].snippet, expected.body);
        assert_eq!(result.has_more, has_more);
        assert!(!serde_json::to_string(&result).unwrap().contains("Old body"));
    }
    responder.await.expect("fake Meili completed");
}
