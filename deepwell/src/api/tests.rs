use super::*;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Semaphore, mpsc};

#[tokio::test]
async fn http_rpc_capacity_rejects_only_excess_active_calls_and_recovers() {
    const ACTIVE: usize = 512;
    let server = rpc_server_builder().build("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", server.local_addr().unwrap());
    let (started_tx, mut started_rx) = mpsc::channel(ACTIVE);
    let release = Arc::new(Semaphore::new(0));
    let mut module = RpcModule::new(());
    module
        .register_async_method("hold", {
            let release = Arc::clone(&release);
            move |_, _, _| {
                let started_tx = started_tx.clone();
                let release = Arc::clone(&release);
                async move {
                    started_tx.send(()).await.unwrap();
                    release.acquire().await.unwrap().forget();
                    "released"
                }
            }
        })
        .unwrap();
    let handle = server.start(module);
    let client = ReqwestClient::builder().build().unwrap();
    let mut calls = Vec::with_capacity(ACTIVE);
    for _ in 0..ACTIVE {
        let client = client.clone();
        let endpoint = endpoint.clone();
        calls.push(tokio::spawn(async move {
            client
                .post(endpoint)
                .json(&json!({"jsonrpc": "2.0", "id": 1, "method": "hold", "params": []}))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }));
    }
    tokio::time::timeout(Duration::from_secs(15), async {
        for _ in 0..ACTIVE {
            started_rx.recv().await.expect("held call must start");
        }
    })
    .await
    .expect("server rejected calls before 512 active RPCs");

    let excess = client
        .post(&endpoint)
        .json(&json!({"jsonrpc": "2.0", "id": 2, "method": "hold", "params": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(excess.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);

    release.add_permits(ACTIVE);
    for call in calls {
        assert_eq!(call.await.unwrap()["result"], "released");
    }
    release.add_permits(1);
    assert_eq!(
        request_rpc(&client, &endpoint, "hold", json!([])).await["result"],
        "released"
    );
    handle.stop().unwrap();
    handle.stopped().await;
}

#[tokio::test]
async fn idle_http_keepalives_do_not_consume_rpc_capacity() {
    let server = rpc_server_builder().build("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let mut module = RpcModule::new(());
    module.register_method("ping", |_, _, _| "pong").unwrap();
    let handle = server.start(module);
    let mut idle = Vec::new();
    for _ in 0..120 {
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut response = [0; 1024];
        let bytes = socket.read(&mut response).await.unwrap();
        assert!(bytes > 0);
        idle.push(socket);
    }
    let endpoint = format!("http://{address}");
    let client = ReqwestClient::new();
    assert_eq!(
        request_rpc(&client, &endpoint, "ping", json!([])).await["result"],
        "pong"
    );
    assert_eq!(idle.len(), 120);
    handle.stop().unwrap();
    handle.stopped().await;
}

async fn request_rpc(
    client: &ReqwestClient,
    endpoint: &str,
    method: &str,
    params: Value,
) -> Value {
    client
        .post(endpoint)
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
        .send()
        .await
        .expect("RPC transport failed")
        .error_for_status()
        .expect("RPC HTTP request failed")
        .json()
        .await
        .expect("RPC response was not JSON")
}

#[tokio::test]
async fn site_domain_accepts_wws_positional_params_over_http() {
    let state = build_server_state(Config::integration_testing(), Secrets::load())
        .await
        .expect("Integration services must be running and seeded");
    let server = rpc_server_builder()
        .set_http_middleware(tower::ServiceBuilder::new().layer(RequestContextLayer))
        .build("127.0.0.1:0")
        .await
        .unwrap();
    let endpoint = format!("http://{}", server.local_addr().unwrap());
    let handle = server.start(build_module(state).await.unwrap());
    let client = ReqwestClient::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    for (slug, expected_domain) in [
        ("test", "test.wikijump.com"),
        ("scp-wiki", "scpwiki.localhost"),
    ] {
        let site =
            request_rpc(&client, &endpoint, "site_get", json!({"site": slug})).await;
        let site_id = site["result"]["site_id"]
            .as_i64()
            .expect("Seeded site missing");
        let response =
            request_rpc(&client, &endpoint, "site_domain", json!([site_id])).await;
        assert_eq!(
            response,
            json!({"jsonrpc": "2.0", "id": 1, "result": expected_domain})
        );
    }

    for params in [
        json!([]),
        json!(["test"]),
        json!([1, 2]),
        json!({"site_id": 1}),
    ] {
        let response = request_rpc(&client, &endpoint, "site_domain", params).await;
        assert_eq!(response["error"]["code"], -32602);
        assert!(response.get("result").is_none());
    }

    handle.stop().unwrap();
    handle.stopped().await;
}
