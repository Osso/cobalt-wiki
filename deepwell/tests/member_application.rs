//! DB-backed membership application lifecycle through session-scoped RPCs.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::prelude::*;
use deepwell::models::relation::{self, Entity as Relation};
use deepwell::models::session::Model as SessionModel;
use deepwell::services::RelationService;
use deepwell::services::RequestContext;
use deepwell::services::ServiceContext;
use deepwell::services::member_application::{
    DecideApplication, MemberApplicationService, SubmitApplication,
};
use deepwell::services::permission::{CheckPermissionContext, PermissionService};
use deepwell::services::relation::RelationObject;
use deepwell::types::{Action, Permission, RelationType, Resource};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde_json::json;
use time::macros::datetime;

async fn site_id(runner: &TestRunner) -> i64 {
    run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id
}

async fn guest(runner: &TestRunner, name: &str) -> i64 {
    run_endpoint!(
        runner,
        user_create,
        json!({
            "user_type": "regular", "name": name,
            "email": format!("{}@example.com", name.to_lowercase()),
            "locales": ["en"], "password": "secret",
            "bypass_filter": true, "bypass_email_verification": true,
            "ip_address": common::IP_ADDRESS,
        }),
    )
    .user_id
}

fn act_as(runner: &mut TestRunner, site_id: i64, user_id: Option<i64>) {
    runner.set_request_context(RequestContext {
        user_id,
        site_id: Some(site_id),
        ..Default::default()
    });
}

fn submit(message: &str) -> serde_json::Value {
    json!({"message": message, "ip_address": common::IP_ADDRESS})
}

fn decide(user_id: i64, accept: bool) -> serde_json::Value {
    json!({"user_id": user_id, "accept": accept, "ip_address": common::IP_ADDRESS})
}

async fn can_edit(runner: &TestRunner, site_id: i64, user_id: i64) -> bool {
    PermissionService::check_user_can(
        runner.context(),
        &CheckPermissionContext {
            user_id: Some(user_id),
            site_id,
            page_reference: None,
        },
        Permission {
            resource_type: Resource::Page,
            resource_category: None,
            action: Action::Edit,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn application_stays_guest_until_admin_accepts() {
    let mut runner = TestRunner::setup().await;
    let site = site_id(&runner).await;
    let applicant = guest(&runner, "ApplicationGuestOne").await;
    act_as(&mut runner, site, Some(applicant));
    let empty = run_endpoint!(runner, member_application_get, json!({}));
    assert!(!empty.is_member);
    assert!(empty.application.is_none());

    run_endpoint!(
        runner,
        member_application_submit,
        submit("  I can help edit  ")
    );
    let pending = run_endpoint!(runner, member_application_get, json!({}));
    assert!(!pending.is_member);
    assert_eq!(pending.application.unwrap().message, "I can help edit");
    assert!(
        run_endpoint!(
            runner,
            membership_get,
            json!({"site_id":site,"user_id":applicant})
        )
        .is_none()
    );
    assert!(
        run_endpoint!(
            runner,
            get_user_roles,
            json!({"site_id":site,"user_id":applicant})
        )
        .into_iter()
        .all(|role| role.is_virtual)
    );
    assert!(!can_edit(&runner, site, applicant).await);

    act_as(&mut runner, site, Some(ADMIN_USER_ID));
    let pending = run_endpoint!(runner, member_application_list, json!({}));
    let entry = pending
        .iter()
        .find(|entry| entry.user_id == applicant)
        .unwrap();
    assert_eq!(entry.user_name, "ApplicationGuestOne");
    assert_eq!(entry.message, "I can help edit");
    run_endpoint!(runner, member_application_decide, decide(applicant, true));
    assert!(
        !run_endpoint!(runner, member_application_list, json!({}))
            .iter()
            .any(|entry| entry.user_id == applicant)
    );

    act_as(&mut runner, site, Some(applicant));
    let accepted = run_endpoint!(runner, member_application_get, json!({}));
    assert!(accepted.is_member);
    assert!(accepted.application.is_none());
    let membership = run_endpoint!(
        runner,
        membership_get,
        json!({"site_id":site,"user_id":applicant})
    )
    .unwrap();
    assert_eq!(
        membership.metadata["accepted"],
        json!({"cause":"accepted", "user_id": ADMIN_USER_ID})
    );
    let roles = run_endpoint!(
        runner,
        get_user_roles,
        json!({"site_id":site,"user_id":applicant})
    );
    assert!(
        roles
            .iter()
            .any(|role| role.name == "member" && !role.is_virtual)
    );
    assert!(can_edit(&runner, site, applicant).await);
    assert_contains_error!(
        run_endpoint_err!(runner, member_application_submit, submit("again")),
        ErrorType::SiteMemberExists
    );
}

#[tokio::test]
async fn rejection_allows_reapplication_and_decision_consumes_once() {
    let mut runner = TestRunner::setup().await;
    let site = site_id(&runner).await;
    let applicant = guest(&runner, "ApplicationGuestTwo").await;
    act_as(&mut runner, site, Some(applicant));
    run_endpoint!(runner, member_application_submit, submit("first"));
    assert_contains_error!(
        run_endpoint_err!(runner, member_application_submit, submit("duplicate")),
        ErrorType::BadRequest
    );
    act_as(&mut runner, site, Some(ADMIN_USER_ID));
    run_endpoint!(runner, member_application_decide, decide(applicant, false));
    assert_contains_error!(
        run_endpoint_err!(runner, member_application_decide, decide(applicant, true)),
        ErrorType::BadRequest
    );
    act_as(&mut runner, site, Some(applicant));
    assert!(
        run_endpoint!(runner, member_application_get, json!({}))
            .application
            .is_none()
    );
    run_endpoint!(runner, member_application_submit, submit("second"));
    assert_eq!(
        run_endpoint!(runner, member_application_get, json!({}))
            .application
            .unwrap()
            .message,
        "second"
    );
}

#[tokio::test]
async fn invalid_messages_and_unauthorized_calls_leave_no_application() {
    let mut runner = TestRunner::setup().await;
    let site = site_id(&runner).await;
    let applicant = guest(&runner, "ApplicationGuestThree").await;
    act_as(&mut runner, site, Some(applicant));
    for message in [" \n  ".to_owned(), "x".repeat(2001)] {
        assert_contains_error!(
            run_endpoint_err!(runner, member_application_submit, submit(&message)),
            ErrorType::BadRequest
        );
    }
    assert!(
        run_endpoint!(runner, member_application_get, json!({}))
            .application
            .is_none()
    );
    assert_contains_error!(
        run_endpoint_err!(runner, member_application_list, json!({})),
        ErrorType::PermissionDenied
    );
    assert_contains_error!(
        run_endpoint_err!(runner, member_application_decide, decide(applicant, true)),
        ErrorType::PermissionDenied
    );

    // A system identity must not be allowed to apply as a regular guest.
    act_as(&mut runner, site, Some(-2));
    assert_contains_error!(
        run_endpoint_err!(runner, member_application_submit, submit("system")),
        ErrorType::PermissionDenied
    );

    act_as(&mut runner, site, None);
    for error in [
        run_endpoint_err!(runner, member_application_get, json!({})),
        run_endpoint_err!(runner, member_application_submit, submit("hello")),
    ] {
        assert_contains_error!(error, ErrorType::PermissionDenied);
    }
    let restricted = RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site),
        session: Some(SessionModel {
            session_token: "restricted-application-session".into(),
            user_id: ADMIN_USER_ID,
            created_at: datetime!(2026-09-24 00:00 UTC),
            expires_at: datetime!(2099-01-01 00:00 UTC),
            ip_address: common::IP_ADDRESS.to_string(),
            user_agent: "test".into(),
            restricted: true,
        }),
        ..Default::default()
    };
    runner.set_request_context(restricted);
    assert_contains_error!(
        run_endpoint_err!(runner, member_application_get, json!({})),
        ErrorType::PermissionDenied
    );
    assert_contains_error!(
        run_endpoint_err!(runner, member_application_submit, submit("hello")),
        ErrorType::PermissionDenied
    );
    assert_contains_error!(
        run_endpoint_err!(runner, member_application_list, json!({})),
        ErrorType::PermissionDenied
    );
    assert_contains_error!(
        run_endpoint_err!(runner, member_application_decide, decide(applicant, true)),
        ErrorType::PermissionDenied
    );
}

#[tokio::test]
async fn malformed_persisted_message_is_reported_not_hidden() {
    let mut runner = TestRunner::setup().await;
    let site = site_id(&runner).await;
    let applicant = guest(&runner, "ApplicationGuestMalformed").await;
    RelationService::create(
        runner.context(),
        RelationType::SiteApplication,
        RelationObject::Site(site),
        RelationObject::User(applicant),
        applicant,
        &json!({}),
    )
    .await
    .unwrap();
    act_as(&mut runner, site, Some(applicant));
    assert_contains_error!(
        run_endpoint_err!(runner, member_application_get, json!({})),
        ErrorType::DatabaseQuery
    );
    act_as(&mut runner, site, Some(ADMIN_USER_ID));
    assert_contains_error!(
        run_endpoint_err!(runner, member_application_list, json!({})),
        ErrorType::DatabaseQuery
    );
}

#[tokio::test]
async fn session_site_and_user_override_untrusted_params() {
    let mut runner = TestRunner::setup().await;
    let site = site_id(&runner).await;
    let other_site = run_endpoint!(runner, site_get, json!({"site": "scp-wiki"}))
        .unwrap()
        .site
        .site_id;
    let applicant = guest(&runner, "ApplicationGuestFour").await;
    act_as(&mut runner, site, Some(applicant));
    run_endpoint!(
        runner,
        member_application_submit,
        json!({
            "message": "site-specific", "ip_address": common::IP_ADDRESS,
            "site_id": other_site, "user_id": ADMIN_USER_ID,
        })
    );
    assert!(
        run_endpoint!(
            runner,
            member_application_get,
            json!({"site_id":other_site,"user_id":ADMIN_USER_ID})
        )
        .application
        .is_some()
    );
    act_as(&mut runner, other_site, Some(applicant));
    assert!(
        run_endpoint!(
            runner,
            member_application_get,
            json!({"site_id":site,"user_id":ADMIN_USER_ID})
        )
        .application
        .is_none()
    );
    act_as(&mut runner, site, Some(ADMIN_USER_ID));
    assert!(
        run_endpoint!(
            runner,
            member_application_list,
            json!({"site_id":other_site})
        )
        .iter()
        .any(|entry| entry.user_id == applicant)
    );
}

// Separate database transactions, unlike TestRunner's per-test transaction:
// both requests compete on the same persisted applicant row.
#[tokio::test]
async fn concurrent_submissions_and_reviews_consume_one_pending_relation() {
    let runner = TestRunner::setup().await;
    let state = runner.state();
    let site = site_id(&runner).await;
    let guest = -6; // Seeded regular guest; not a site member.
    let guest_request = RequestContext {
        user_id: Some(guest),
        site_id: Some(site),
        ..Default::default()
    };
    let admin_request = RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site),
        ..Default::default()
    };
    let first_tx = state.database.begin().await.unwrap();
    let second_tx = state.database.begin().await.unwrap();
    let input = || SubmitApplication {
        message: "concurrent".into(),
        ip_address: common::IP_ADDRESS,
    };
    let first = ServiceContext::new(state, &first_tx).with_request(guest_request.clone());
    let first_result = MemberApplicationService::submit(&first, input()).await;
    assert!(first_result.is_ok(), "first submit: {first_result:?}");
    drop(first);
    let ((), second_result) = tokio::join!(
        async {
            tokio::task::yield_now().await;
            first_tx.commit().await.unwrap()
        },
        async {
            let second =
                ServiceContext::new(state, &second_tx).with_request(guest_request);
            let result = MemberApplicationService::submit(&second, input()).await;
            drop(second);
            second_tx.rollback().await.unwrap();
            result
        },
    );
    assert_contains_error!(second_result.unwrap_err(), ErrorType::BadRequest);

    let application_id = Relation::find()
        .filter(relation::Column::RelationType.eq(RelationType::SiteApplication))
        .filter(relation::Column::DestId.eq(site))
        .filter(relation::Column::FromId.eq(guest))
        .one(&state.database)
        .await
        .unwrap()
        .unwrap()
        .relation_id;
    let first_tx = state.database.begin().await.unwrap();
    let second_tx = state.database.begin().await.unwrap();
    let input = || DecideApplication {
        user_id: guest,
        accept: false,
        ip_address: common::IP_ADDRESS,
    };
    let first = ServiceContext::new(state, &first_tx).with_request(admin_request.clone());
    let first_result = MemberApplicationService::decide(&first, input()).await;
    assert!(first_result.is_ok(), "first review: {first_result:?}");
    drop(first);
    let ((), second_result) = tokio::join!(
        async {
            tokio::task::yield_now().await;
            first_tx.commit().await.unwrap()
        },
        async {
            let second =
                ServiceContext::new(state, &second_tx).with_request(admin_request);
            let result = MemberApplicationService::decide(&second, input()).await;
            drop(second);
            second_tx.rollback().await.unwrap();
            result
        },
    );
    assert_contains_error!(second_result.unwrap_err(), ErrorType::BadRequest);

    // Remove only this test's relation; leave the seeded guest unchanged.
    let cleanup = state.database.begin().await.unwrap();
    Relation::delete_by_id(application_id)
        .exec(&cleanup)
        .await
        .unwrap();
    cleanup.commit().await.unwrap();
}
