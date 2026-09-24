//! Set-password links: a trusted caller creates one for a member (an invite),
//! anyone asks for one by email address (a forgotten password), and the
//! link's holder chooses a password with it once, before it expires.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::error::exn_error_to_rpc_error;
use deepwell::error::prelude::*;
use deepwell::models::password_token;
use deepwell::services::AuthenticationService;
use deepwell::services::authentication::AuthenticateUser;
use deepwell::services::email::MailgunSender;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use sea_query::Expr;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const EMAIL: &str = "pw-link-member@example.com";
const OLD_PASSWORD: &str = "random nobody is told";

async fn site_id(runner: &TestRunner) -> i64 {
    run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id
}

async fn create_member(runner: &TestRunner, name: &str, email: &str) -> i64 {
    run_endpoint!(
        runner,
        user_create,
        json!({
            "user_type": "regular", "name": name, "email": email,
            "locales": ["en"], "password": OLD_PASSWORD,
            "bypass_filter": true, "bypass_email_verification": true,
            "ip_address": common::IP_ADDRESS,
        }),
    )
    .user_id
}

async fn create_link(runner: &TestRunner, user_id: i64, site_id: i64) -> String {
    let output = run_endpoint!(
        runner,
        password_token_create,
        json!({"user": user_id, "site_id": site_id}),
    );
    assert!(!output.emailed);
    output
        .path
        .strip_prefix("/-/set-password/")
        .expect("link path")
        .to_owned()
}

fn rpc_code(error: ExnError) -> i32 {
    exn_error_to_rpc_error(error).code()
}

async fn redeem_code(runner: &TestRunner, token: &str, password: &str) -> i32 {
    rpc_code(run_endpoint_err!(
        runner,
        password_token_redeem,
        json!({"token": token, "password": password, "ip_address": common::IP_ADDRESS}),
    ))
}

/// The password check of login, without the session it would create.
async fn can_login(runner: &TestRunner, name: &str, password: &str) -> bool {
    AuthenticationService::auth_password(
        runner.context(),
        AuthenticateUser {
            name_or_email: name.into(),
            password: password.into(),
        },
    )
    .await
    .is_ok()
}

#[tokio::test]
async fn link_sets_password_once_and_newest_link_replaces_older() {
    let runner = TestRunner::setup().await;
    let site_id = site_id(&runner).await;
    let user_id = create_member(&runner, "PwLinkMember", EMAIL).await;

    let first = create_link(&runner, user_id, site_id).await;
    let second = create_link(&runner, user_id, site_id).await;
    assert_eq!(second.len(), 43);
    assert_ne!(first, second);

    let before = run_endpoint!(runner, password_token_status, json!({"user": user_id}));
    assert!(!before.password_chosen);
    assert!(!before.pending.as_ref().expect("pending link").emailed);

    // Stored as a hash only: no row holds the token text.
    let rows = password_token::Entity::find()
        .filter(password_token::Column::UserId.eq(user_id))
        .all(runner.context().transaction())
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "creating a link removed the unused older one"
    );
    assert_ne!(rows[0].token_hash, second.as_bytes());

    assert_eq!(redeem_code(&runner, &first, "chosen secret").await, 3007);
    assert_eq!(redeem_code(&runner, &second, "").await, 3005);
    assert!(can_login(&runner, "pwlinkmember", OLD_PASSWORD).await);

    run_endpoint!(
        runner,
        password_token_redeem,
        json!({"token": second, "password": "chosen secret", "ip_address": common::IP_ADDRESS}),
    );
    assert!(can_login(&runner, "pwlinkmember", "chosen secret").await);
    assert!(!can_login(&runner, "pwlinkmember", OLD_PASSWORD).await);

    assert_eq!(redeem_code(&runner, &second, "another secret").await, 3009);
    assert!(can_login(&runner, "pwlinkmember", "chosen secret").await);
    assert_eq!(redeem_code(&runner, "not-a-real-token", "x").await, 3007);

    let after = run_endpoint!(runner, password_token_status, json!({"user": user_id}));
    assert_eq!(
        serde_json::to_value(after).unwrap(),
        json!({"password_chosen": true, "pending": null}),
    );
}

#[tokio::test]
async fn expired_link_is_refused_and_changes_nothing() {
    let runner = TestRunner::setup().await;
    let site_id = site_id(&runner).await;
    let user_id = create_member(&runner, "PwLinkExpired", EMAIL).await;
    let token = create_link(&runner, user_id, site_id).await;

    let past = time::OffsetDateTime::now_utc() - time::Duration::days(8);
    password_token::Entity::update_many()
        .col_expr(password_token::Column::CreatedAt, Expr::value(past))
        .col_expr(
            password_token::Column::ExpiresAt,
            Expr::value(past + time::Duration::days(7)),
        )
        .filter(password_token::Column::UserId.eq(user_id))
        .exec(runner.context().transaction())
        .await
        .unwrap();

    assert_eq!(redeem_code(&runner, &token, "chosen secret").await, 3008);
    assert!(can_login(&runner, "pwlinkexpired", OLD_PASSWORD).await);
    let status = run_endpoint!(runner, password_token_status, json!({"user": user_id}));
    assert!(!status.password_chosen);
    assert!(status.pending.is_none(), "an expired link is not pending");
}

#[tokio::test]
async fn emailing_needs_mailgun_and_a_deliverable_address() {
    let runner = TestRunner::setup().await;
    let site_id = site_id(&runner).await;
    let member = create_member(&runner, "PwLinkMail", EMAIL).await;
    let placeholder =
        create_member(&runner, "PwLinkPlaceholder", "wikidot-1@members.invalid").await;

    for user in [member, placeholder] {
        let error = run_endpoint_err!(
            runner,
            password_token_create,
            json!({"user": user, "site_id": site_id, "send_email": true}),
        );
        if user == member {
            assert_contains_error!(error, ErrorType::EmailSend);
        } else {
            assert_eq!(rpc_code(error), ErrorType::BadRequest.code());
        }
    }
    let status = run_endpoint!(runner, password_token_status, json!({"user": member}));
    assert!(status.pending.is_none(), "a failed send leaves no link");
}

#[tokio::test]
async fn member_email_update_can_skip_verification() {
    let runner = TestRunner::setup().await;
    let user_id = create_member(&runner, "PwLinkEmailEdit", EMAIL).await;

    // The integration config's mocked MailCheck rejects invalid.com.
    let edit = |bypass: bool| {
        json!({
            "user": user_id, "email": "member@invalid.com",
            "bypass_filter": true, "bypass_email_verification": bypass,
            "ip_address": common::IP_ADDRESS,
        })
    };
    run_endpoint_err!(runner, user_edit, edit(false));
    let user = run_endpoint!(runner, user_edit, edit(true));
    assert_eq!(user.email, "member@invalid.com");
    assert_eq!(user.email_validation_info, None);
}

/// Records the raw requests Mailgun would receive, answering each with 200.
async fn fake_mailgun() -> (MailgunSender, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let size = socket.read(&mut chunk).await.unwrap();
                bytes.extend_from_slice(&chunk[..size]);
                let text = String::from_utf8_lossy(&bytes);
                if let Some((header, body)) = text.split_once("\r\n\r\n") {
                    let length: usize = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|value| value.trim().parse().ok())
                        })
                        .unwrap_or(0);
                    if body.len() >= length {
                        break;
                    }
                }
            }
            seen.lock().unwrap().push(String::from_utf8(bytes).unwrap());
            let body = r#"{"message":"Queued. Thank you."}"#;
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len(),
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let sender = MailgunSender::new(
        url,
        "mg.example.org".into(),
        "key-test".into(),
        "Cobalt Company <noreply@mg.example.org>".into(),
    );
    (sender, requests)
}

fn form_field(request: &str, name: &str) -> String {
    let body = request.split_once("\r\n\r\n").unwrap().1;
    form_urlencoded::parse(body.as_bytes())
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
        .unwrap_or_default()
}

async fn wait_for_requests(requests: &Mutex<Vec<String>>, count: usize) -> Vec<String> {
    for _ in 0..50 {
        if requests.lock().unwrap().len() >= count {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // Long enough for an unexpected extra email to arrive too.
    tokio::time::sleep(Duration::from_millis(200)).await;
    requests.lock().unwrap().clone()
}

#[tokio::test]
async fn forgotten_password_answers_alike_and_emails_only_real_accounts() {
    let (sender, requests) = fake_mailgun().await;
    let runner = TestRunner::setup_with_mailgun(sender).await;
    let site_id = site_id(&runner).await;
    create_member(&runner, "PwLinkForgot", EMAIL).await;

    let request = |email: &str| json!({"email": email, "site_id": site_id});
    let unknown = run_endpoint!(
        runner,
        password_reset_request,
        request("nobody-here@example.com")
    );
    let known = run_endpoint!(
        runner,
        password_reset_request,
        request("PW-Link-Member@Example.com")
    );
    assert_eq!(
        serde_json::to_value(unknown).unwrap(),
        serde_json::to_value(known).unwrap()
    );

    let sent = wait_for_requests(&requests, 1).await;
    assert_eq!(sent.len(), 1, "only the existing account gets an email");
    assert_eq!(form_field(&sent[0], "to"), EMAIL);
    assert_eq!(
        form_field(&sent[0], "subject"),
        "Reset your password for Test site"
    );
    let text = form_field(&sent[0], "text");
    let token = text
        .split_once("https://test.wikijump.com/-/set-password/")
        .expect("link in email")
        .1
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();

    // A second request right away sends nothing more.
    run_endpoint!(runner, password_reset_request, request(EMAIL));
    assert_eq!(wait_for_requests(&requests, 2).await.len(), 1);

    run_endpoint!(
        runner,
        password_token_redeem,
        json!({"token": token, "password": "remembered now", "ip_address": common::IP_ADDRESS}),
    );
    assert!(can_login(&runner, "pwlinkforgot", "remembered now").await);
}

#[tokio::test]
async fn forgotten_password_without_mailgun_fails_alike_for_every_address() {
    let runner = TestRunner::setup().await;
    let site_id = site_id(&runner).await;
    create_member(&runner, "PwLinkNoMail", EMAIL).await;

    for email in [EMAIL, "nobody-here@example.com"] {
        let error = run_endpoint_err!(
            runner,
            password_reset_request,
            json!({"email": email, "site_id": site_id}),
        );
        assert_contains_error!(error, ErrorType::EmailSend);
    }
}
