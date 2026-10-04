//! Sessions only grant what authentication completed: MFA-pending sessions
//! cannot be renewed into full ones, and deleted accounts lose all access.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::config::Config;
use deepwell::error::prelude::*;
use deepwell::services::SessionService;
use deepwell::services::session::CreateSession;
use serde_json::json;

const PASSWORD: &str = "lifecycle secret";

async fn setup() -> TestRunner {
    let mut config = Config::integration_testing();
    config.session_token_length = 64; // the session table requires > 48
    TestRunner::setup_with_config(config).await
}

async fn create_user(runner: &TestRunner, name: &str) -> i64 {
    run_endpoint!(
        runner,
        user_create,
        json!({
            "user_type": "regular", "name": name, "email": format!("{name}@example.com"),
            "locales": ["en"], "password": PASSWORD,
            "bypass_filter": true, "bypass_email_verification": true,
            "ip_address": common::IP_ADDRESS,
        }),
    )
    .user_id
}

async fn create_session(runner: &TestRunner, user_id: i64, restricted: bool) -> String {
    SessionService::create(
        runner.context(),
        CreateSession {
            user_id,
            ip_address: common::IP_ADDRESS,
            user_agent: "auth-lifecycle".into(),
            restricted,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn pending_mfa_session_cannot_be_renewed_into_a_full_session() {
    let runner = setup().await;
    let user_id = create_user(&runner, "renewpending").await;
    let pending = create_session(&runner, user_id, true).await;

    let renew = |token: &str| {
        json!([{
            "old_session_token": token, "user_id": user_id,
            "ip_address": common::IP_ADDRESS, "user_agent": "auth-lifecycle",
        }])
    };
    let error = run_endpoint_err!(runner, auth_session_renew, renew(&pending));
    assert_contains_error!(error, ErrorType::InvalidSessionToken);
    let session = SessionService::get(runner.context(), &pending)
        .await
        .unwrap();
    assert!(session.restricted, "pending session stays pending");

    let full = create_session(&runner, user_id, false).await;
    let renewed: String = run_endpoint!(runner, auth_session_renew, renew(&full));
    let session = SessionService::get(runner.context(), &renewed)
        .await
        .unwrap();
    assert_eq!((session.user_id, session.restricted), (user_id, false));
}

#[tokio::test]
async fn deleted_user_cannot_log_in_and_loses_sessions() {
    let runner = setup().await;
    let user_id = create_user(&runner, "deletedlogin").await;
    let session = create_session(&runner, user_id, false).await;
    let login = json!({
        "name_or_email": "deletedlogin", "password": PASSWORD,
        "ip_address": common::IP_ADDRESS, "user_agent": "auth-lifecycle",
    });
    run_endpoint!(runner, auth_login, login.clone());

    run_endpoint!(runner, user_delete, json!({"user": user_id}));

    let error = run_endpoint_err!(runner, auth_login, login);
    assert_contains_error!(error, ErrorType::InvalidAuthentication);
    assert!(
        SessionService::get_optional(runner.context(), &session)
            .await
            .unwrap()
            .is_none(),
        "deleted user's session ended"
    );
}
