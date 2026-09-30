//! Account color preferences persist through the existing user endpoints.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::endpoints::all::{user_create, user_edit, user_get};
use deepwell::models::{known_user, user};
use deepwell::services::ServiceContext;
use sea_orm::{ConnectionTrait, EntityTrait, TransactionTrait};
use serde_json::{Value, json};

async fn read_profile(runner: &TestRunner, user_id: i64) -> Value {
    let transaction = runner.state().database.begin().await.unwrap();
    let context = ServiceContext::new(runner.state(), &transaction);
    let output = user_get(&context, common::make_params(json!({"user": user_id})))
        .await
        .expect("read user in a separate transaction")
        .expect("user exists");
    let user = output.user.unwrap_wikijump().expect("native account");
    transaction.commit().await.unwrap();
    serde_json::to_value(user).unwrap()["color_profile"].clone()
}

#[test]
fn color_profile_input_is_strict() {
    use deepwell::services::user::UpdateUserBody;

    for profile in ["classic", "current"] {
        assert!(
            serde_json::from_value::<UpdateUserBody>(json!({"color_profile": profile}))
                .is_ok()
        );
    }
    for invalid in [
        json!("dark"),
        json!("Classic"),
        json!(""),
        json!(null),
        json!(1),
    ] {
        assert!(
            serde_json::from_value::<UpdateUserBody>(json!({"color_profile": invalid}))
                .is_err(),
            "invalid color preference must be rejected"
        );
    }
}

#[tokio::test]
async fn defaults_and_edits_survive_separate_transactions() {
    let url = std::env::var("DATABASE_URL").expect("dedicated test database URL");
    assert!(
        url.contains(":25432/cobalt_test"),
        "dedicated test database only"
    );
    let runner = TestRunner::setup().await;
    assert_eq!(read_profile(&runner, ADMIN_USER_ID).await, "current");

    let transaction = runner.state().database.begin().await.unwrap();
    let context = ServiceContext::new(runner.state(), &transaction);
    let name = format!("Color {}", uuid::Uuid::new_v4().simple());
    let created = user_create(
        &context,
        common::make_params(json!({
            "user_type": "regular", "name": name,
            "email": "color-profile@example.com", "locales": ["en"],
            "password": "color-profile-test-password",
            "bypass_filter": true, "bypass_email_verification": true,
            "ip_address": common::IP_ADDRESS,
        })),
    )
    .await
    .expect("create account without a color preference");
    transaction.commit().await.unwrap();
    assert_eq!(read_profile(&runner, created.user_id).await, "current");

    for profile in ["classic", "current", "classic"] {
        let transaction = runner.state().database.begin().await.unwrap();
        let context = ServiceContext::new(runner.state(), &transaction);
        let edited = user_edit(
            &context,
            common::make_params(json!({
                "user": created.user_id, "color_profile": profile,
                "ip_address": common::IP_ADDRESS,
            })),
        )
        .await
        .expect("edit color preference");
        assert_eq!(
            serde_json::to_value(edited).unwrap()["color_profile"],
            profile
        );
        transaction.commit().await.unwrap();
        assert_eq!(read_profile(&runner, created.user_id).await, profile);
    }

    let transaction = runner.state().database.begin().await.unwrap();
    let context = ServiceContext::new(runner.state(), &transaction);
    user_edit(
        &context,
        common::make_params(json!({
            "user": created.user_id, "biography": "Unrelated profile edit",
            "ip_address": common::IP_ADDRESS,
        })),
    )
    .await
    .expect("edit without a color preference");
    transaction.commit().await.unwrap();
    assert_eq!(read_profile(&runner, created.user_id).await, "classic");
    assert_eq!(read_profile(&runner, ADMIN_USER_ID).await, "current");

    for invalid in [
        json!("dark"),
        json!("Classic"),
        json!(""),
        json!(null),
        json!(1),
    ] {
        let transaction = runner.state().database.begin().await.unwrap();
        let context = ServiceContext::new(runner.state(), &transaction);
        assert!(
            user_edit(
                &context,
                common::make_params(json!({
                    "user": created.user_id, "color_profile": invalid,
                    "ip_address": common::IP_ADDRESS,
                })),
            )
            .await
            .is_err(),
            "invalid color preference must be rejected"
        );
        transaction.rollback().await.unwrap();
        assert_eq!(read_profile(&runner, created.user_id).await, "classic");
    }

    for invalid in ["'dark'", "NULL"] {
        let transaction = runner.state().database.begin().await.unwrap();
        assert!(
            transaction
                .execute_unprepared(&format!(
                    "UPDATE \"user\" SET color_profile = {invalid} WHERE user_id = {}",
                    created.user_id
                ))
                .await
                .is_err(),
            "database must reject invalid color preferences"
        );
        transaction.rollback().await.unwrap();
        assert_eq!(read_profile(&runner, created.user_id).await, "classic");
    }

    let transaction = runner.state().database.begin().await.unwrap();
    user::Entity::delete_by_id(created.user_id)
        .exec(&transaction)
        .await
        .unwrap();
    known_user::Entity::delete_by_id(created.user_id)
        .exec(&transaction)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
}
