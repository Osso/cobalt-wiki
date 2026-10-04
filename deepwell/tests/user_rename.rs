//! A user's past names stay theirs: another user cannot rename onto them.

#[macro_use]
mod common;

use self::common::TestRunner;
use deepwell::error::prelude::*;
use serde_json::json;

async fn create_user(runner: &TestRunner, name: &str, email: &str) -> i64 {
    run_endpoint!(
        runner,
        user_create,
        json!({
            "user_type": "regular",
            "name": name,
            "email": email,
            "locales": ["en"],
            "password": "hunter2",
            "ip_address": common::IP_ADDRESS,
        }),
    )
    .user_id
}

async fn rename(runner: &TestRunner, user_id: i64, name: &str) {
    run_endpoint!(
        runner,
        user_edit,
        json!({"user": user_id, "name": name, "ip_address": common::IP_ADDRESS}),
    );
}

async fn user_by_slug(runner: &TestRunner, slug: &str) -> (i64, String, i16) {
    let user = run_endpoint!(runner, user_get, json!({ "user": slug }))
        .expect("user should resolve")
        .user
        .unwrap_wikijump()
        .expect("user should be a Wikijump user");
    (user.user_id, user.slug, user.name_changes_left)
}

#[tokio::test]
async fn rename_cannot_take_another_users_past_name() {
    let runner = TestRunner::setup().await;
    let bob = create_user(&runner, "Bob Original", "bob@private.me").await;
    let alice = create_user(&runner, "Alice Original", "alice@private.me").await;
    rename(&runner, bob, "Robert Renamed").await;

    let error = run_endpoint_err!(
        runner,
        user_edit,
        json!({"user": alice, "name": "Bob Original", "ip_address": common::IP_ADDRESS}),
    );
    assert_contains_error!(error, ErrorType::UserExists);

    assert_eq!(
        user_by_slug(&runner, "bob-original").await,
        (bob, "robert-renamed".to_owned(), 1),
        "Bob's old name still resolves to Bob",
    );
    assert_eq!(
        user_by_slug(&runner, "alice-original").await,
        (alice, "alice-original".to_owned(), 2),
        "Alice is unchanged",
    );

    // The owner may still return to their own past name for free.
    rename(&runner, bob, "Bob Original").await;
    assert_eq!(
        user_by_slug(&runner, "bob-original").await,
        (bob, "bob-original".to_owned(), 1),
    );
}
