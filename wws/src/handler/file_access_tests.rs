//! Unignored Deepwell RPC timeout tests use local HTTP. File route tests use
//! the real router and Redis; run those with `WWS_TEST_REDIS_URL` and `--ignored`.

use crate::config::Secrets;
use crate::deepwell::Deepwell;
use crate::error::Error;
use crate::route::build_router;
use crate::state::build_server_state;
use axum::Router;
use axum::body::Body;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_SECURITY_POLICY, CONTENT_TYPE, COOKIE,
    ETAG, LOCATION,
};
use axum::http::{Method, Request, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use jsonrpsee::core::ClientError;
use s3::creds::Credentials;
use s3::region::Region;
use serde_json::{Value, json};
use std::env;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tower::ServiceExt;

const PUBLIC_PAGE_ID: i64 = 101;
const PRIVATE_PAGE_ID: i64 = 202;
const MEMBER_TOKEN: &str = "wj:member-session";
const R2_ENDPOINT: &str = "https://account.r2.cloudflarestorage.com";

/// Answers the RPCs wws makes for a file request, like DEEPWELL would for
/// a public multi-colon page and the private `admin:css`.
async fn fake_deepwell(body: String) -> impl IntoResponse {
    let request: Value = serde_json::from_str(&body).unwrap();
    let params = &request["params"];
    let result = match request["method"].as_str().unwrap() {
        "page_get" => match params["page"].as_str().unwrap() {
            "writing:2021-10-21-to-paint-a-picture:the-game" => {
                json!({ "page_id": PUBLIC_PAGE_ID })
            }
            "admin:css" => json!({ "page_id": PRIVATE_PAGE_ID }),
            _ => Value::Null,
        },
        "page_view_permission" => match params["page_id"].as_i64().unwrap() {
            PRIVATE_PAGE_ID => json!({
                "can_view": params["session_token"] == MEMBER_TOKEN,
                "public": false,
            }),
            _ => json!({ "can_view": true, "public": true }),
        },
        "file_get" => json!({
            "file_id": 1,
            "mime": if params["file"] == "chessset.jpg" { "image/jpeg" } else { "font/woff2" },
            "size": 4,
            "s3_hash": format!("hash-of-{}", params["file"].as_str().unwrap()),
        }),
        method => panic!("unexpected DEEPWELL call {method}"),
    };
    let response = json!({ "jsonrpc": "2.0", "id": request["id"], "result": result });
    ([(CONTENT_TYPE, "application/json")], response.to_string())
}

async fn start_fake_deepwell() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/", post(fake_deepwell));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}/")
}

/// A local HTTP JSON-RPC endpoint with delayed permission and file replies.
async fn delayed_deepwell(body: String) -> Response<Body> {
    let request: Value = serde_json::from_str(&body).unwrap();
    if request["params"]["session_token"] == "stall" {
        tokio::time::sleep(Duration::from_millis(5_500)).await;
    } else {
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    if request["params"]["file"] == "missing.jpg" {
        let response = json!({ "jsonrpc": "2.0", "id": request["id"], "result": null });
        return ([(CONTENT_TYPE, "application/json")], response.to_string())
            .into_response();
    }
    if request["params"]["file"] == "error.jpg" {
        let response = json!({ "jsonrpc": "2.0", "id": request["id"], "error": {
            "code": -32000, "message": "file metadata unavailable"
        } });
        return ([(CONTENT_TYPE, "application/json")], response.to_string())
            .into_response();
    }
    fake_deepwell(body).await.into_response()
}

async fn start_delayed_deepwell() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/", post(delayed_deepwell));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}/")
}

#[tokio::test]
async fn delayed_deepwell_serves_205_concurrent_permission_and_file_requests() {
    let deepwell = Arc::new(Deepwell::connect(&start_delayed_deepwell().await).unwrap());
    let mut requests = JoinSet::new();
    for index in 0..205 {
        let deepwell = Arc::clone(&deepwell);
        requests.spawn(async move {
            let permission = deepwell
                .get_page_view_permission(1, PUBLIC_PAGE_ID, None)
                .await
                .unwrap();
            assert!(permission.can_view && permission.public);
            let filename = format!("pet-icon-{index}.jpg");
            let file = deepwell
                .get_file(1, PUBLIC_PAGE_ID, &filename)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(file.s3_hash, format!("hash-of-{filename}"));
        });
    }
    let mut completed = 0;
    while let Some(result) = requests.join_next().await {
        result.unwrap();
        completed += 1;
    }
    assert_eq!(completed, 205);
}

#[tokio::test]
async fn delayed_deepwell_keeps_denial_missing_file_and_rpc_errors() {
    let deepwell = Deepwell::connect(&start_delayed_deepwell().await).unwrap();
    let denied = deepwell
        .get_page_view_permission(1, PRIVATE_PAGE_ID, None)
        .await
        .unwrap();
    assert!(!denied.can_view && !denied.public);
    assert!(
        deepwell
            .get_file(1, PUBLIC_PAGE_ID, "missing.jpg")
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        deepwell.get_file(1, PUBLIC_PAGE_ID, "error.jpg").await,
        Err(Error::Deepwell(ClientError::Call(_)))
    ));
}

#[tokio::test]
async fn delayed_deepwell_still_times_out_when_backend_stalls() {
    let deepwell = Deepwell::connect(&start_delayed_deepwell().await).unwrap();
    assert!(matches!(
        deepwell
            .get_page_view_permission(1, PUBLIC_PAGE_ID, Some("stall"))
            .await,
        Err(Error::Deepwell(ClientError::RequestTimeout))
    ));
}

async fn start_replacement_sources()
-> (String, String, Arc<std::sync::atomic::AtomicBool>) {
    use std::sync::atomic::{AtomicBool, Ordering};

    let replaced = Arc::new(AtomicBool::new(false));
    let current = Arc::clone(&replaced);
    let rpc = Router::new().route(
        "/",
        post(move |body: String| {
            let current = Arc::clone(&current);
            async move {
                let request: Value = serde_json::from_str(&body).unwrap();
                if request["method"] != "file_get" {
                    return fake_deepwell(body).await.into_response();
                }
                let (id, hash) = if current.load(Ordering::SeqCst) {
                    (2, "replacement")
                } else {
                    (1, "original")
                };
                let response = json!({"jsonrpc": "2.0", "id": request["id"], "result": {
                    "file_id": id, "mime": "image/jpeg", "size": 4, "s3_hash": hash
                }});
                ([(CONTENT_TYPE, "application/json")], response.to_string())
                    .into_response()
            }
        }),
    );
    let storage = Router::new().route(
        "/wikijump-files/{hash}",
        axum::routing::get(
            |axum::extract::Path(hash): axum::extract::Path<String>| async move {
                match hash.as_str() {
                    "original" => "old!".into_response(),
                    "replacement" => "new!".into_response(),
                    _ => StatusCode::NOT_FOUND.into_response(),
                }
            },
        ),
    );
    let rpc_url = start_http_fixture(rpc).await;
    let storage_url = start_http_fixture(storage).await;
    (rpc_url, storage_url, replaced)
}

const ACTIVE_HTML: &str = include_str!("fixtures/active-document.html");
const ACTIVE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(document.cookie)</script></svg>"#;

async fn active_content_router() -> Router {
    let rpc = Router::new().route(
        "/",
        post(|body: String| async move {
            let request: Value = serde_json::from_str(&body).unwrap();
            let result = match request["method"].as_str().unwrap() {
                "file_get" => {
                    let (mime, bytes) =
                        active_object(request["params"]["file"].as_str().unwrap());
                    json!({"file_id": 1, "mime": mime, "size": bytes.len(),
                    "s3_hash": request["params"]["file"]})
                }
                "user_get" => json!({"avatar_s3_hash": [171]}),
                _ => return fake_deepwell(body).await.into_response(),
            };
            (
                [(CONTENT_TYPE, "application/json")],
                json!({"jsonrpc": "2.0", "id": request["id"], "result": result})
                    .to_string(),
            )
                .into_response()
        }),
    );
    let storage = Router::new().route(
        "/wikijump-files/{key}",
        axum::routing::get(
            |axum::extract::Path(key): axum::extract::Path<String>| async move {
                let (mime, bytes) = active_object(&key);
                ([(CONTENT_TYPE, mime)], bytes)
            },
        ),
    );
    router_with_sources(
        start_http_fixture(rpc).await,
        start_http_fixture(storage).await,
    )
    .await
}

fn active_object(key: &str) -> (&'static str, &'static [u8]) {
    match key {
        "active.html" | "ab" => ("text/html", ACTIVE_HTML.as_bytes()),
        "active.svg" => ("image/svg+xml", ACTIVE_SVG.as_bytes()),
        "pixel.png" => ("image/png", include_bytes!("fixtures/pixel.png")),
        "font.woff2" => ("font/woff2", b"wOF2"),
        "notes.txt" => ("text/plain", b"ordinary text\n"),
        _ => panic!("unexpected object {key}"),
    }
}

#[tokio::test]
#[ignore = "needs isolated Redis at WWS_TEST_REDIS_URL"]
async fn public_active_file_document_is_sandboxed() {
    let router = active_content_router().await;
    let path = "/-/file/writing:2021-10-21-to-paint-a-picture:the-game/active.html";
    let response = request(&router, Method::GET, unique_site_id(), path, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, CONTENT_TYPE), "text/html");
    assert_eq!(header(&response, CACHE_CONTROL), "public, no-cache");
    assert_eq!(header(&response, ETAG), "\"active.html\"");
    assert!(response.headers().get(CONTENT_DISPOSITION).is_none());
    let headers: serde_json::Map<String, Value> = response
        .headers()
        .iter()
        .map(|(key, value)| (key.to_string(), json!(value.to_str().unwrap())))
        .collect();
    let bytes = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    assert_eq!(bytes, ACTIVE_HTML);
    // Export before the isolation assertion so Chromium can reproduce the RED contract.
    if let Ok(filename) = env::var("WWS_FILE_FIXTURE") {
        std::fs::write(
            filename,
            json!({"path": path, "headers": headers,
            "body": String::from_utf8(bytes.to_vec()).unwrap()})
            .to_string(),
        )
        .unwrap();
    }
    assert_eq!(
        headers.get("content-security-policy"),
        Some(&json!("sandbox allow-scripts"))
    );
}

#[tokio::test]
#[ignore = "needs isolated Redis at WWS_TEST_REDIS_URL"]
async fn document_isolation_keeps_file_bytes_types_and_download_disposition() {
    let router = active_content_router().await;
    let site_id = unique_site_id();
    for filename in [
        "active.html",
        "active.svg",
        "pixel.png",
        "font.woff2",
        "notes.txt",
    ] {
        let (mime, bytes) = active_object(filename);
        for route in ["file", "download"] {
            let path = format!(
                "/-/{route}/writing:2021-10-21-to-paint-a-picture:the-game/{filename}"
            );
            let response = request(&router, Method::GET, site_id, &path, None).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(header(&response, CONTENT_TYPE), mime);
            assert_eq!(header(&response, CACHE_CONTROL), "public, no-cache");
            assert_eq!(header(&response, ETAG), format!("\"{filename}\""));
            if route == "download" {
                assert_eq!(
                    header(&response, CONTENT_DISPOSITION),
                    format!("attachment; filename=\"{filename}\"")
                );
            } else {
                assert!(response.headers().get(CONTENT_DISPOSITION).is_none());
            }
            assert_eq!(
                header(&response, CONTENT_SECURITY_POLICY),
                "sandbox allow-scripts"
            );
            assert_eq!(
                axum::body::to_bytes(response.into_body(), 8192)
                    .await
                    .unwrap(),
                bytes
            );
        }
    }
}

#[tokio::test]
#[ignore = "needs isolated Redis at WWS_TEST_REDIS_URL"]
async fn private_file_accepts_browser_encoded_session_cookie() {
    let router = active_content_router().await;
    let site_id = unique_site_id();
    for route in ["file", "download"] {
        let path = format!("/-/{route}/admin:css/pixel.png");
        let response = request(
            &router,
            Method::GET,
            site_id,
            &path,
            Some("wj%3Amember-session"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(header(&response, CACHE_CONTROL), "private, no-store");
        assert!(
            header(&response, LOCATION).contains("response-content-type=image%2Fpng")
        );
        for invalid in [
            None,
            Some("wj%253Amember-session"),
            Some("wj%3Aother"),
            Some("%FF"),
        ] {
            let denied = request(&router, Method::GET, site_id, &path, invalid).await;
            assert_eq!(denied.status(), StatusCode::FORBIDDEN);
            assert_eq!(header(&denied, CACHE_CONTROL), "private, no-store");
            assert!(denied.headers().get(LOCATION).is_none());
        }
    }
}

#[tokio::test]
#[ignore = "needs isolated Redis at WWS_TEST_REDIS_URL"]
async fn active_private_file_keeps_denial_and_presigned_redirect() {
    let router = active_content_router().await;
    let site_id = unique_site_id();
    let path = "/-/file/admin:css/active.html";
    for session in [None, Some("wj:other")] {
        let denied = request(&router, Method::GET, site_id, path, session).await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        assert_eq!(header(&denied, CACHE_CONTROL), "private, no-store");
        assert!(denied.headers().get(LOCATION).is_none());
        assert_eq!(
            axum::body::to_bytes(denied.into_body(), 8192)
                .await
                .unwrap(),
            "Forbidden"
        );
    }
    for route in ["file", "download"] {
        let response = request(
            &router,
            Method::GET,
            site_id,
            &format!("/-/{route}/admin:css/active.html"),
            Some(MEMBER_TOKEN),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(header(&response, CACHE_CONTROL), "private, no-store");
        let location = header(&response, LOCATION);
        assert!(
            location.contains("response-content-type=text%2Fhtml"),
            "{location}"
        );
        let disposition = if route == "download" {
            "attachment"
        } else {
            "inline"
        };
        assert!(location.contains(&format!("response-content-disposition={disposition}%3B%20filename%3D%22active.html%22")), "{location}");
        assert!(location.contains("X-Amz-Expires=604800"), "{location}");
        assert_eq!(
            header(&response, CONTENT_SECURITY_POLICY),
            "sandbox allow-scripts"
        );
    }
}

#[tokio::test]
#[ignore = "needs isolated Redis at WWS_TEST_REDIS_URL"]
async fn avatar_metadata_html_is_sandboxed_without_changing_bytes() {
    let router = active_content_router().await;
    let response = request(
        &router,
        Method::GET,
        unique_site_id(),
        "/-/avatar/876543210",
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, CONTENT_TYPE), "text/html");
    assert_eq!(header(&response, ETAG), "\"ab\"");
    assert_eq!(
        header(&response, CONTENT_SECURITY_POLICY),
        "sandbox allow-scripts"
    );
    assert_eq!(
        axum::body::to_bytes(response.into_body(), 8192)
            .await
            .unwrap(),
        ACTIVE_HTML
    );
}

#[tokio::test]
#[ignore = "needs isolated Redis at WWS_TEST_REDIS_URL"]
async fn avatar_missing_from_storage_is_an_error_response_not_a_panic() {
    let rpc = Router::new().route(
        "/",
        post(|body: String| async move {
            let request: Value = serde_json::from_str(&body).unwrap();
            let result = match request["method"].as_str().unwrap() {
                "user_get" => json!({"avatar_s3_hash": [205]}),
                "basic_error_blob_fetch" => {
                    json!({"title": "Missing", "body": "<p>Missing</p>"})
                }
                method => panic!("unexpected DEEPWELL call {method}"),
            };
            (
                [(CONTENT_TYPE, "application/json")],
                json!({"jsonrpc": "2.0", "id": request["id"], "result": result})
                    .to_string(),
            )
        }),
    );
    let storage = Router::new().fallback(|| async { StatusCode::NOT_FOUND });
    let router = router_with_sources(
        start_http_fixture(rpc).await,
        start_http_fixture(storage).await,
    )
    .await;
    let path = format!("/-/avatar/{}", unique_site_id());
    let response = request(&router, Method::GET, unique_site_id(), &path, None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

async fn start_http_fixture(app: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

#[tokio::test]
#[ignore = "needs Redis at WWS_TEST_REDIS_URL"]
async fn same_filename_replacement_serves_current_bytes_after_cache_warmup() {
    use std::sync::atomic::Ordering;

    let (rpc, storage, replaced) = start_replacement_sources().await;
    let router = router_with_sources(rpc, storage).await;
    let site_id = unique_site_id();
    let path = "/-/file/writing:2021-10-21-to-paint-a-picture:the-game/chessset.jpg";
    let original = request(&router, Method::GET, site_id, path, None).await;
    assert_eq!(original.status(), StatusCode::OK);
    assert_eq!(header(&original, ETAG), "\"original\"");
    assert_eq!(
        axum::body::to_bytes(original.into_body(), 100)
            .await
            .unwrap(),
        "old!"
    );

    // Replacement creates a different file ID under the same filename, as in production.
    replaced.store(true, Ordering::SeqCst);
    let replacement = request(&router, Method::GET, site_id, path, None).await;
    assert_eq!(replacement.status(), StatusCode::OK);
    assert_eq!(header(&replacement, ETAG), "\"replacement\"");
    assert_eq!(header(&replacement, CACHE_CONTROL), "public, no-cache");
    assert_eq!(
        axum::body::to_bytes(replacement.into_body(), 100)
            .await
            .unwrap(),
        "new!"
    );
}

#[tokio::test]
#[ignore = "needs isolated Redis at WWS_TEST_REDIS_URL"]
async fn missing_filename_is_not_cached_before_rename_makes_it_available() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let renamed = Arc::new(AtomicBool::new(false));
    let current = Arc::clone(&renamed);
    let rpc = Router::new().route(
        "/",
        post(move |body: String| {
            let current = Arc::clone(&current);
            async move {
                let request: Value = serde_json::from_str(&body).unwrap();
                let result = match request["method"].as_str().unwrap() {
                    "file_get" => {
                        let name = if current.load(Ordering::SeqCst) {
                            "icon_zath.jpg"
                        } else {
                            "icon_tyrrell.jpg"
                        };
                        if request["params"]["file"] == name {
                            json!({"file_id": 1405, "mime": "image/jpeg", "size": 4,
                                "s3_hash": "unchanged"})
                        } else {
                            Value::Null
                        }
                    }
                    "basic_error_missing_file_name" => {
                        json!({"title": "Missing file", "body": "<p>Missing file</p>"})
                    }
                    _ => return fake_deepwell(body).await.into_response(),
                };
                (
                    [(CONTENT_TYPE, "application/json")],
                    json!({"jsonrpc": "2.0", "id": request["id"], "result": result})
                        .to_string(),
                )
                    .into_response()
            }
        }),
    );
    let storage = Router::new().route(
        "/wikijump-files/unchanged",
        axum::routing::get(|| async { "jpeg" }),
    );
    let router = router_with_sources(
        start_http_fixture(rpc).await,
        start_http_fixture(storage).await,
    )
    .await;
    let site_id = unique_site_id();
    let owner = "writing:2021-10-21-to-paint-a-picture:the-game";
    for route in ["file", "download"] {
        let path = format!("/-/{route}/{owner}/icon_zath.jpg");
        let missing = request(&router, Method::GET, site_id, &path, None).await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            missing
                .headers()
                .get(CACHE_CONTROL)
                .map(|value| value.to_str().unwrap()),
            Some("no-store"),
            "a pre-rename 404 must not hide a subsequently renamed attachment"
        );
    }

    renamed.store(true, Ordering::SeqCst);
    for route in ["file", "download"] {
        let path = format!("/-/{route}/{owner}/icon_zath.jpg");
        let response = request(&router, Method::GET, site_id, &path, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(header(&response, CACHE_CONTROL), "public, no-cache");
        assert_eq!(header(&response, ETAG), "\"unchanged\"");
        assert_eq!(
            axum::body::to_bytes(response.into_body(), 100)
                .await
                .unwrap(),
            "jpeg"
        );
        let old_path = format!("/-/{route}/{owner}/icon_tyrrell.jpg");
        let retired = request(&router, Method::GET, site_id, &old_path, None).await;
        assert_eq!(retired.status(), StatusCode::NOT_FOUND);
        assert_eq!(header(&retired, CACHE_CONTROL), "no-store");
    }
}

/// Fresh site ID per run, so earlier runs' cached pages and files never match.
fn unique_site_id() -> i64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    9_000_000_000 + (nanos % 1_000_000_000) as i64
}

async fn router() -> Router {
    router_with_sources(start_fake_deepwell().await, str!(R2_ENDPOINT)).await
}

async fn router_with_sources(deepwell_url: String, endpoint: String) -> Router {
    let redis_url = env::var("WWS_TEST_REDIS_URL")
        .expect("WWS_TEST_REDIS_URL must name a Redis database");
    let secrets = Secrets {
        deepwell_url,
        redis_url,
        s3_files_bucket: str!("wikijump-files"),
        s3_tblocks_bucket: str!("wikijump-text-blocks"),
        s3_region: Region::Custom {
            region: str!("auto"),
            endpoint,
        },
        s3_path_style: true,
        // AWS documentation example keys.
        s3_credentials: Credentials::new(
            Some("AKIAIOSFODNN7EXAMPLE"),
            Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"),
            None,
            None,
            None,
        )
        .unwrap(),
    };
    build_router(build_server_state(false, secrets).await.unwrap())
}

async fn request(
    router: &Router,
    method: Method,
    site_id: i64,
    path: &str,
    session: Option<&str>,
) -> Response<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("x-wikijump-site-id", site_id)
        .header("x-wikijump-target-server", "main");
    if let Some(token) = session {
        builder = builder.header(COOKIE, format!("theme=dark; wikijump_token={token}"));
    }
    router
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn header<'a>(
    response: &'a Response<Body>,
    name: impl axum::http::header::AsHeaderName,
) -> &'a str {
    response.headers().get(name).unwrap().to_str().unwrap()
}

#[tokio::test]
#[ignore = "needs Redis at WWS_TEST_REDIS_URL"]
async fn private_page_file_redirects_viewer_and_refuses_anonymous() {
    let router = router().await;
    let site_id = unique_site_id();
    let file = "/-/file/admin:css/liberty-webfont.woff2";

    let anonymous = request(&router, Method::GET, site_id, file, None).await;
    assert_eq!(anonymous.status(), StatusCode::FORBIDDEN);
    assert_eq!(header(&anonymous, CACHE_CONTROL), "private, no-store");
    assert!(anonymous.headers().get(LOCATION).is_none());

    let stranger = request(&router, Method::GET, site_id, file, Some("wj:other")).await;
    assert_eq!(stranger.status(), StatusCode::FORBIDDEN);

    let member = request(&router, Method::GET, site_id, file, Some(MEMBER_TOKEN)).await;
    assert_eq!(member.status(), StatusCode::FOUND);
    assert_eq!(header(&member, CACHE_CONTROL), "private, no-store");
    let location = header(&member, LOCATION);
    assert!(
        location.starts_with(&format!(
            "{R2_ENDPOINT}/wikijump-files/hash-of-liberty-webfont.woff2?"
        )),
        "{location}",
    );
    assert!(
        location.contains("response-content-type=font%2Fwoff2"),
        "{location}"
    );
    assert!(
        location.contains("response-content-disposition=inline%3B%20filename%3D%22liberty-webfont.woff2%22"),
        "{location}",
    );
    assert!(location.contains("X-Amz-Expires=604800"), "{location}");

    let again = request(&router, Method::GET, site_id, file, Some(MEMBER_TOKEN)).await;
    assert_eq!(
        header(&again, LOCATION),
        location,
        "same URL within the day"
    );

    let download = request(
        &router,
        Method::GET,
        site_id,
        "/-/download/admin:css/liberty-webfont.woff2",
        Some(MEMBER_TOKEN),
    )
    .await;
    assert_eq!(download.status(), StatusCode::FOUND);
    assert!(
        header(&download, LOCATION)
            .contains("response-content-disposition=attachment%3B"),
        "{}",
        header(&download, LOCATION),
    );
}

#[tokio::test]
#[ignore = "needs Redis at WWS_TEST_REDIS_URL"]
async fn public_multi_colon_page_file_requires_revalidation() {
    let router = router().await;
    let site_id = unique_site_id();
    let file = "/-/file/writing:2021-10-21-to-paint-a-picture:the-game/chessset.jpg";

    // HEAD takes the streaming path without fetching the object body.
    for session in [None, Some(MEMBER_TOKEN)] {
        let response = request(&router, Method::HEAD, site_id, file, session).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(header(&response, CACHE_CONTROL), "public, no-cache");
        assert_eq!(header(&response, ETAG), "\"hash-of-chessset.jpg\"");
        assert_eq!(header(&response, CONTENT_TYPE), "image/jpeg");
        assert!(response.headers().get(LOCATION).is_none());
    }

    let download = request(
        &router,
        Method::HEAD,
        site_id,
        "/-/download/writing:2021-10-21-to-paint-a-picture:the-game/chessset.jpg",
        None,
    )
    .await;
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(
        header(&download, CONTENT_DISPOSITION),
        "attachment; filename=\"chessset.jpg\"",
    );
}
