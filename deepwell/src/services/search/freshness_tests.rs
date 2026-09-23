use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn same_revision_rerender_is_hidden_before_visible_pagination() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let documents: Vec<_> = (1..=3)
        .map(|page_id| SearchDocument {
            page_id,
            site_id: 7,
            revision_id: page_id * 10,
            title: format!("Page {page_id}"),
            slug: format!("page-{page_id}"),
            tags: vec![],
            body: if page_id == 1 {
                "old private body".into()
            } else {
                format!("current body {page_id}")
            },
        })
        .collect();
    let response = serde_json::json!({"hits": documents}).to_string();
    tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 4096];
            socket.read(&mut bytes).await.unwrap();
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let service = SearchService::new(url, "key".into());
    for (offset, expected_id) in [(0, 2), (1, 3)] {
        let page = service
            .search_with(
                7,
                SearchRequest {
                    query: "body".into(),
                    offset,
                    limit: 1,
                },
                |doc| async move {
                    // Revision IDs are unchanged; only the stored compiled HTML changed.
                    let current_html = match doc.page_id {
                        1 => "<p>replacement public body</p>".to_string(),
                        id => format!("<p>current body {id}</p>"),
                    };
                    Ok(SearchHit::from_current_document(&doc, &current_html))
                },
            )
            .await
            .unwrap();
        assert_eq!(page.hits.len(), 1);
        assert_eq!(page.hits[0].page_id, expected_id);
        assert_eq!(page.hits[0].snippet, format!("current body {expected_id}"));
        assert_eq!(page.has_more, offset == 0);
        assert!(
            !serde_json::to_string(&page)
                .unwrap()
                .contains("old private body")
        );
    }
}
