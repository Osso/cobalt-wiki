//! File routes end to end: the real router and Redis cache against a fake
//! DEEPWELL. Run with `WWS_TEST_REDIS_URL=redis://host:port/db cargo test -- --ignored`.

use crate::config::Secrets;
use crate::route::build_router;
use crate::state::build_server_state;
use axum::Router;
use axum::body::Body;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE, COOKIE, ETAG, LOCATION,
};
use axum::http::{Method, Request, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use s3::creds::Credentials;
use s3::region::Region;
use serde_json::{Value, json};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
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

/// Fresh site ID per run, so earlier runs' cached pages and files never match.
fn unique_site_id() -> i64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    9_000_000_000 + (nanos % 1_000_000_000) as i64
}

async fn router() -> Router {
    let redis_url = env::var("WWS_TEST_REDIS_URL")
        .expect("WWS_TEST_REDIS_URL must name a Redis database");
    let secrets = Secrets {
        deepwell_url: start_fake_deepwell().await,
        redis_url,
        s3_files_bucket: str!("wikijump-files"),
        s3_tblocks_bucket: str!("wikijump-text-blocks"),
        s3_region: Region::Custom {
            region: str!("auto"),
            endpoint: str!(R2_ENDPOINT),
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
async fn public_multi_colon_page_file_is_served_with_long_public_cache() {
    let router = router().await;
    let site_id = unique_site_id();
    let file = "/-/file/writing:2021-10-21-to-paint-a-picture:the-game/chessset.jpg";

    // HEAD takes the streaming path without fetching the object body.
    for session in [None, Some(MEMBER_TOKEN)] {
        let response = request(&router, Method::HEAD, site_id, file, session).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(header(&response, CACHE_CONTROL), "public, max-age=2592000");
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
