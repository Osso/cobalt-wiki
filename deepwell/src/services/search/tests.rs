use super::*;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn fixture(responses: Vec<String>) -> (SearchService, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    tokio::spawn(async move {
        for response in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let header_end = loop {
                let mut chunk = [0; 4096];
                let size = socket.read(&mut chunk).await.unwrap();
                assert!(size > 0);
                bytes.extend_from_slice(&chunk[..size]);
                if let Some(position) =
                    bytes.windows(4).position(|part| part == b"\r\n\r\n")
                {
                    break position + 4;
                }
            };
            let header = String::from_utf8_lossy(&bytes[..header_end]);
            let length: usize = header
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .and_then(|value| value.trim().parse().ok())
                })
                .unwrap_or(0);
            while bytes.len() < header_end + length {
                let mut chunk = [0; 4096];
                let size = socket.read(&mut chunk).await.unwrap();
                assert!(size > 0);
                bytes.extend_from_slice(&chunk[..size]);
            }
            seen.lock().unwrap().push(String::from_utf8(bytes).unwrap());
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    (SearchService::new(url, "private-key".into()), requests)
}

#[tokio::test]
async fn visibility_precedes_paging_and_no_restricted_totals_escape() {
    let first: Vec<_> = (1..=50)
        .map(|id| SearchDocument {
            page_id: id,
            site_id: 7,
            revision_id: id * 10,
            title: format!("Page {id}"),
            slug: format!("page-{id}"),
            tags: vec![],
            body: if id == 1 {
                "classified".into()
            } else {
                "hello".into()
            },
        })
        .collect();
    let second: Vec<_> = (51..=53)
        .map(|id| SearchDocument {
            page_id: id,
            site_id: if id == 51 { 8 } else { 7 },
            revision_id: id * 10,
            title: format!("Page {id}"),
            slug: format!("page-{id}"),
            tags: vec![],
            body: "hello".into(),
        })
        .collect();
    let (service, requests) = fixture(vec![
        serde_json::json!({"hits": first, "estimatedTotalHits": 53}).to_string(),
        serde_json::json!({"hits": second, "estimatedTotalHits": 53}).to_string(),
        serde_json::json!({"hits": first, "estimatedTotalHits": 53}).to_string(),
        serde_json::json!({"hits": second, "estimatedTotalHits": 53}).to_string(),
    ])
    .await;
    let first_page = service
        .search_with(
            7,
            SearchRequest {
                query: "hello".into(),
                offset: 0,
                limit: 1,
            },
            |doc| async move {
                if [3, 52, 53].contains(&doc.page_id) {
                    Ok(Some(SearchHit::from_document(&doc)))
                } else {
                    Ok(None)
                }
            },
        )
        .await
        .unwrap();
    assert_eq!(first_page.hits[0].page_id, 3);
    assert!(first_page.has_more);
    let page = service
        .search_with(
            7,
            SearchRequest {
                query: "hello".into(),
                offset: 1,
                limit: 1,
            },
            |doc| async move {
                if [3, 52, 53].contains(&doc.page_id) {
                    Ok(Some(SearchHit::from_document(&doc)))
                } else {
                    Ok(None)
                }
            },
        )
        .await
        .unwrap();
    assert_eq!(
        page.hits.iter().map(|hit| hit.page_id).collect::<Vec<_>>(),
        vec![52]
    );
    assert!(page.has_more);
    let wire = requests.lock().unwrap();
    assert_eq!(wire.len(), 4);
    for request in wire.iter() {
        assert!(request.contains("\"filter\":\"site_id = 7\""));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer private-key")
        );
    }
    assert!(!serde_json::to_string(&page).unwrap().contains("classified"));
}

#[tokio::test]
async fn upsert_and_remove_use_idempotent_meili_tasks() {
    let (service, requests) = fixture(vec![
        r#"{"taskUid":21}"#.into(),
        r#"{"uid":21,"status":"succeeded"}"#.into(),
        r#"{"taskUid":22}"#.into(),
        r#"{"uid":22,"status":"succeeded"}"#.into(),
    ])
    .await;
    let document = SearchDocument {
        page_id: 4,
        site_id: 7,
        revision_id: 10,
        title: "Title".into(),
        slug: "title".into(),
        tags: vec!["tag".into()],
        body: "visible body".into(),
    };
    service.upsert(document).await.unwrap();
    service.remove_page(4).await.unwrap();
    let wire = requests.lock().unwrap();
    assert!(
        wire[0].starts_with("POST /indexes/pages/documents?primaryKey=page_id HTTP/1.1")
    );
    assert!(wire[0].contains("\"body\":\"visible body\""));
    assert!(wire[2].starts_with("DELETE /indexes/pages/documents/4 HTTP/1.1"));
}

#[tokio::test]
async fn existing_index_configuration_is_repeatable() {
    let (service, requests) = fixture(vec![
        r#"{"uid":"pages","primaryKey":"page_id"}"#.into(),
        r#"{"taskUid":31}"#.into(),
        r#"{"uid":31,"status":"succeeded"}"#.into(),
    ])
    .await;
    service.ensure_index().await.unwrap();
    let requests = requests.lock().unwrap();
    assert!(requests[0].starts_with("GET /indexes/pages HTTP/1.1"));
    assert!(requests[1].starts_with("PATCH /indexes/pages/settings HTTP/1.1"));
    let settings: serde_json::Value =
        serde_json::from_str(requests[1].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(
        settings["filterableAttributes"],
        serde_json::json!(["site_id"])
    );
    assert_eq!(
        settings["searchableAttributes"],
        serde_json::json!(["title", "slug", "tags", "body"])
    );
}

#[test]
fn html_plain_text_omits_nonvisible_markup() {
    let html = "<p>Hello &amp; goodbye</p><script>secret()</script><style>.secret{}</style><div hidden>hidden</div><span class=\"wj-hidden\">conditional secret</span><p>Visible <b>text</b></p>";
    assert_eq!(plain_body(html), "Hello & goodbye Visible text");
}
