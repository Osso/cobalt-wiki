//! Sessions slide: using a session pushes its expiry out to a full
//! `normal_session_duration`, so a signed-in member is only logged out
//! after that long without any use.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::config::Config;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::session::{self, Entity as Session};
use deepwell::services::SessionService;
use deepwell::services::session::CreateSession;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::json;
use time::{Duration, OffsetDateTime};

/// The test configuration's 16-character tokens fail the session table's
/// length check, so these sessions need real-length tokens.
async fn setup() -> TestRunner {
    let mut config = Config::integration_testing();
    config.session_token_length = 64;
    TestRunner::setup_with_config(config).await
}

async fn create_session(runner: &TestRunner, restricted: bool) -> String {
    SessionService::create(
        runner.context(),
        CreateSession {
            user_id: ADMIN_USER_ID,
            ip_address: common::IP_ADDRESS,
            user_agent: "session-sliding-test".into(),
            restricted,
        },
    )
    .await
    .expect("create session")
}

async fn stored_expiry(runner: &TestRunner, token: &str) -> OffsetDateTime {
    Session::find_by_id(token.to_owned())
        .one(runner.context().transaction())
        .await
        .expect("read session")
        .expect("session row")
        .expires_at
}

/// Moves a session's lifetime so it ends at `expires_at`; the table requires
/// the session to be created before it expires.
async fn set_expiry(runner: &TestRunner, token: &str, expires_at: OffsetDateTime) {
    session::ActiveModel {
        session_token: Set(token.to_owned()),
        created_at: Set(expires_at - Duration::hours(1)),
        expires_at: Set(expires_at),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .expect("set session expiry");
}

#[tokio::test]
async fn used_session_near_expiry_is_extended_to_a_full_duration() {
    let runner = setup().await;
    let duration = runner.config().normal_session_duration;
    let token = create_session(&runner, false).await;
    let almost_expired = OffsetDateTime::now_utc() + Duration::seconds(30);
    set_expiry(&runner, &token, almost_expired).await;

    let session = run_endpoint!(runner, auth_session_get, json!([token]))
        .expect("session still active");

    let floor = OffsetDateTime::now_utc() + duration - Duration::minutes(1);
    assert!(session.expires_at > floor, "returned expiry slides forward");
    assert!(
        stored_expiry(&runner, &token).await > floor,
        "stored expiry slides forward"
    );
}

#[tokio::test]
async fn fresh_session_is_not_rewritten_on_every_use() {
    let runner = setup().await;
    let token = create_session(&runner, false).await;
    let created = stored_expiry(&runner, &token).await;

    run_endpoint!(runner, auth_session_get, json!([token])).expect("session active");

    assert_eq!(stored_expiry(&runner, &token).await, created);
}

#[tokio::test]
async fn restricted_login_sessions_do_not_slide() {
    let runner = setup().await;
    let token = create_session(&runner, true).await;
    let almost_expired = OffsetDateTime::now_utc() + Duration::seconds(30);
    set_expiry(&runner, &token, almost_expired).await;
    let before = stored_expiry(&runner, &token).await;

    run_endpoint!(runner, auth_session_get, json!([token]));

    assert_eq!(stored_expiry(&runner, &token).await, before);
}

#[tokio::test]
async fn expired_session_is_not_revived() {
    let runner = setup().await;
    let token = create_session(&runner, false).await;
    let expired = OffsetDateTime::now_utc() - Duration::minutes(1);
    set_expiry(&runner, &token, expired).await;
    let before = stored_expiry(&runner, &token).await;

    let session = run_endpoint!(runner, auth_session_get, json!([token]));

    assert!(session.is_none());
    assert_eq!(stored_expiry(&runner, &token).await, before);
}
