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
    for _ in 0..512 {
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
    for socket in &idle {
        let mut probe = [0; 1];
        assert!(
            matches!(socket.try_read(&mut probe), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
            "idle HTTP connection must remain open with no pending response"
        );
    }
    let endpoint = format!("http://{address}");
    let client = ReqwestClient::new();
    assert_eq!(
        request_rpc(&client, &endpoint, "ping", json!([])).await["result"],
        "pong"
    );
    assert_eq!(idle.len(), 512);
    handle.stop().unwrap();
    handle.stopped().await;
}

#[tokio::test]
async fn mfa_pending_session_has_no_viewer_or_request_actor_until_verified() {
    use crate::services::PasswordService;
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    const USER_ID: i64 = -9000042;
    const PASSWORD: &str = "security-fixture-password";
    const RECOVERY_CODE: &str = "security-recovery-code";
    let mut config = Config::integration_testing();
    config.session_token_length = 64;
    config.job_min_poll_delay = Duration::from_secs(3600);
    config.job_max_poll_delay = Duration::from_secs(3600);
    let state = build_server_state(config, Secrets::load()).await.unwrap();
    let password_hash = PasswordService::new_hash(PASSWORD).unwrap();
    let recovery_hash = PasswordService::new_hash(RECOVERY_CODE).unwrap();
    state
        .database
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO known_user (user_id) VALUES ($1)",
            [USER_ID.into()],
        ))
        .await
        .unwrap();
    state.database.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "INSERT INTO \"user\" (user_id, name, slug, email, password, name_changes_left, locales, multi_factor_secret, multi_factor_recovery_codes) VALUES ($1, 'Security MFA Fixture', 'security-mfa-fixture', 'mfa@example.invalid', $2, 2, ARRAY['en'], 'JBSWY3DPEHPK3PXP', ARRAY[$3])",
        [USER_ID.into(), password_hash.into(), recovery_hash.into()],
    )).await.unwrap();

    let server = rpc_server_builder()
        .set_http_middleware(tower::ServiceBuilder::new().layer(RequestContextLayer))
        .build("127.0.0.1:0")
        .await
        .unwrap();
    let endpoint = format!("http://{}", server.local_addr().unwrap());
    let mut module = build_module(Arc::clone(&state)).await.unwrap();
    register_test_actor(&mut module);
    let handle = server.start(module);
    let client = ReqwestClient::new();
    let login = request_rpc(
        &client,
        &endpoint,
        "login",
        json!({
            "name_or_email": "security-mfa-fixture", "password": PASSWORD,
            "ip_address": "127.0.0.1", "user_agent": "security-regression"
        }),
    )
    .await;
    assert_eq!(login["result"]["needs_mfa"], true, "{login}");
    let pending = login["result"]["session_token"].as_str().unwrap();
    let raw = request_rpc(&client, &endpoint, "session_get", json!([pending])).await;
    assert_eq!(raw["result"]["restricted"], true);
    let pending_actor = request_with_session(
        &client,
        &endpoint,
        pending,
        "security_test_actor",
        json!([]),
    )
    .await;
    let pending_view = request_with_session(
        &client,
        &endpoint,
        pending,
        "user_view",
        json!({
            "site_id": 1, "locales": ["en"], "session_token": pending
        }),
    )
    .await;
    assert_eq!(
        pending_actor["result"]["user_id"],
        Value::Null,
        "{pending_actor}"
    );
    assert_eq!(
        pending_actor["result"]["restricted"], true,
        "MFA retains its raw session"
    );
    assert_eq!(
        pending_view["result"]["type"], "user_missing",
        "{pending_view}"
    );

    let verified = request_rpc(
        &client,
        &endpoint,
        "mfa_verify",
        json!({
            "session_token": pending, "totp_or_code": RECOVERY_CODE,
            "ip_address": "127.0.0.1", "user_agent": "security-regression"
        }),
    )
    .await;
    let token = verified["result"]
        .as_str()
        .unwrap_or_else(|| panic!("correct MFA must complete login: {verified}"));
    let actor =
        request_with_session(&client, &endpoint, token, "security_test_actor", json!([]))
            .await;
    assert_eq!(actor["result"]["user_id"], USER_ID);
    assert_eq!(actor["result"]["restricted"], false);
    let view = request_with_session(
        &client,
        &endpoint,
        token,
        "user_view",
        json!({
            "site_id": 1, "locales": ["en"], "session_token": token
        }),
    )
    .await;
    assert_eq!(view["result"]["type"], "user_found", "{view}");
    assert_eq!(view["result"]["data"]["user"]["user_id"], USER_ID);
    let consumed = request_rpc(&client, &endpoint, "session_get", json!([pending])).await;
    assert_eq!(consumed["result"], Value::Null);
    handle.stop().unwrap();
    handle.stopped().await;
    state
        .database
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "DELETE FROM session WHERE user_id = $1",
            [USER_ID.into()],
        ))
        .await
        .unwrap();
    state
        .database
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "DELETE FROM \"user\" WHERE user_id = $1",
            [USER_ID.into()],
        ))
        .await
        .unwrap();
    state
        .database
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "DELETE FROM known_user WHERE user_id = $1",
            [USER_ID.into()],
        ))
        .await
        .unwrap();
}

fn register_test_actor(module: &mut RpcModule<ServerState>) {
    module
        .register_async_method("security_test_actor", |_, state, extensions| async move {
            let txn = state.database.begin().await.unwrap();
            let ctx = ServiceContext::new(&state, &txn);
            let headers = extensions
                .get::<RequestContextHeaders>()
                .cloned()
                .expect("HTTP middleware supplies request headers");
            let request = build_request(&ctx, &headers).await.unwrap();
            let result = json!({
                "user_id": request.user_id,
                "restricted": request.session.map(|session| session.restricted),
            });
            txn.commit().await.unwrap();
            result
        })
        .unwrap();
}

async fn request_with_session(
    client: &ReqwestClient,
    endpoint: &str,
    token: &str,
    method: &str,
    params: Value,
) -> Value {
    client
        .post(endpoint)
        .header("X-Deepwell-Session-Token", token)
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
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
