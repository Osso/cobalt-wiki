use super::*;
use serde_json::{Value, json};

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
    let server = Server::builder()
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
