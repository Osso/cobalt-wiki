//! Real WWS router with HTTP Deepwell/S3 fixtures and isolated test Redis.
//! Run with WWS_TEST_REDIS_URL and --ignored. WWS_HTML_FIXTURE exports browser input.

use crate::config::Secrets;
use crate::route::build_router;
use crate::state::build_server_state;
use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, COOKIE, ETAG,
};
use axum::http::{Request, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use s3::creds::Credentials;
use s3::region::Region;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpListener;
use tower::ServiceExt;

const HTML: &str = include_str!("fixtures/active-document.html");
const CODE: &str = "console.log('code remains text');";

async fn fake_deepwell(body: String) -> impl IntoResponse {
    let request: Value = serde_json::from_str(&body).unwrap();
    let params = &request["params"];
    let result = match request["method"].as_str().unwrap() {
        "page_get" => {
            json!({"page_id": if params["page"] == "admin:html" {202} else {101}})
        }
        "page_view_permission" => {
            let public = params["page_id"] == 101;
            json!({"can_view": public || params["session_token"] == "wj:member", "public": public})
        }
        method => panic!("unexpected Deepwell RPC {method}"),
    };
    (
        [(CONTENT_TYPE, "application/json")],
        json!({"jsonrpc": "2.0", "id": request["id"], "result": result}).to_string(),
    )
}

async fn fake_s3(
    State(reads): State<Arc<AtomicUsize>>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    reads.fetch_add(1, Ordering::SeqCst);
    let (content_type, body) = match key.as_str() {
        "101_html_1" | "202_html_1" => ("text/html", HTML),
        "101_code_1" | "202_code_1" => ("text/plain", CODE),
        _ => panic!("unexpected S3 key {key}"),
    };
    ([(CONTENT_TYPE, content_type), (ETAG, "\"fixture\"")], body)
}

async fn start_http(app: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

async fn fixture() -> (Router, Arc<AtomicUsize>) {
    let reads = Arc::new(AtomicUsize::new(0));
    let deepwell_url = start_http(Router::new().route("/", post(fake_deepwell))).await;
    let endpoint = start_http(
        Router::new()
            .route("/wikijump-text-blocks/{key}", get(fake_s3))
            .with_state(Arc::clone(&reads)),
    )
    .await;
    let secrets = Secrets {
        deepwell_url,
        redis_url: std::env::var("WWS_TEST_REDIS_URL").expect("isolated Redis required"),
        s3_files_bucket: "wikijump-files".into(),
        s3_tblocks_bucket: "wikijump-text-blocks".into(),
        s3_region: Region::Custom {
            region: "auto".into(),
            endpoint,
        },
        s3_path_style: true,
        s3_credentials: Credentials::new(
            Some("AKIAIOSFODNN7EXAMPLE"),
            Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"),
            None,
            None,
            None,
        )
        .unwrap(),
    };
    (
        build_router(build_server_state(false, secrets).await.unwrap()),
        reads,
    )
}

async fn request(router: &Router, path: &str, session: Option<&str>) -> Response<Body> {
    let mut request = Request::builder()
        .uri(path)
        .header("x-wikijump-site-id", "987654321")
        .header("x-wikijump-target-server", "main");
    if let Some(token) = session {
        request = request.header(COOKIE, format!("wikijump_token={token}"));
    }
    router
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn assert_html(response: Response<Body>, private: bool, export: bool) {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/html");
    assert_eq!(response.headers()[ETAG], "\"fixture\"");
    if private {
        assert_eq!(response.headers()[CACHE_CONTROL], "private, no-store");
    } else {
        assert!(response.headers().get(CACHE_CONTROL).is_none());
    }
    let headers: serde_json::Map<String, Value> = response
        .headers()
        .iter()
        .map(|(key, value)| (key.to_string(), json!(value.to_str().unwrap())))
        .collect();
    let csp = response.headers().get(CONTENT_SECURITY_POLICY).cloned();
    let bytes = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    assert_eq!(bytes, format!("{HTML}\n"));
    if export && let Ok(path) = std::env::var("WWS_HTML_FIXTURE") {
        std::fs::write(path, json!({"path": "/-/html/public:html/1", "headers": headers, "body": String::from_utf8(bytes.to_vec()).unwrap()}).to_string()).unwrap();
    }
    assert_eq!(
        csp.as_ref().map(|v| v.to_str().unwrap()),
        Some("sandbox allow-scripts")
    );
}

#[tokio::test]
#[ignore = "needs isolated Redis at WWS_TEST_REDIS_URL"]
async fn public_html_document_is_sandboxed() {
    let (router, reads) = fixture().await;
    assert_html(
        request(&router, "/-/html/public:html/1", None).await,
        false,
        true,
    )
    .await;
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
#[ignore = "needs isolated Redis at WWS_TEST_REDIS_URL"]
async fn restricted_html_checks_each_viewer_before_storage() {
    let (router, reads) = fixture().await;
    for session in [None, Some("wj:other")] {
        let denied = request(&router, "/-/html/admin:html/1", session).await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        assert_eq!(denied.headers()[CACHE_CONTROL], "private, no-store");
        assert_eq!(
            axum::body::to_bytes(denied.into_body(), 8192)
                .await
                .unwrap(),
            "Forbidden"
        );
    }
    assert_eq!(reads.load(Ordering::SeqCst), 0);
    assert_html(
        request(&router, "/-/html/admin:html/1", Some("wj:member")).await,
        true,
        false,
    )
    .await;
    let denied = request(&router, "/-/html/admin:html/1", None).await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
#[ignore = "needs isolated Redis at WWS_TEST_REDIS_URL"]
async fn code_blocks_keep_content_and_private_cache_with_document_isolation() {
    let (router, _) = fixture().await;
    for (slug, session, private) in [
        ("public:html", None, false),
        ("admin:html", Some("wj:member"), true),
    ] {
        let response = request(&router, &format!("/-/code/{slug}/1"), session).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], "text/plain");
        assert_eq!(response.headers()[ETAG], "\"fixture\"");
        assert_eq!(
            response.headers()[CONTENT_SECURITY_POLICY],
            "sandbox allow-scripts"
        );
        if private {
            assert_eq!(response.headers()[CACHE_CONTROL], "private, no-store");
        } else {
            assert!(response.headers().get(CACHE_CONTROL).is_none());
        }
        assert_eq!(
            axum::body::to_bytes(response.into_body(), 8192)
                .await
                .unwrap(),
            format!("{CODE}\n")
        );
    }
}
