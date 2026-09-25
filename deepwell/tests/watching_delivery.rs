//! Committed page changes delivered through the actual watcher worker and fake Mailgun.
mod common;

use common::{
    TestRunner, fake_mailgun, fake_mailgun_with_status, form_field, wait_for_requests,
};
use deepwell::constants::SYSTEM_USER_ID;
use deepwell::license::License;
use deepwell::models::{page_revision, site, user};
use deepwell::services::permission::PermissionService;
use deepwell::services::relation::{
    CreateSiteMember, RelationService, SiteMemberAccepted, SiteMemberData,
};
use deepwell::services::role::{
    InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::site::{CreateSite, SiteService};
use deepwell::services::user::{CreateUser, UserService};
use deepwell::services::watching::subscriptions::{WatchPreferences, WatchScope};
use deepwell::services::watching::{
    activity, subscriptions, visibility::visible_change, worker,
};
use deepwell::services::{RequestContext, ServiceContext, TextService};
use deepwell::types::{Action, Permission, Reference, Resource, UserType};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseBackend,
    DatabaseTransaction, Statement, TransactionTrait,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;
use uuid::Uuid;

struct Fixture {
    site_id: i64,
    author: i64,
    author_name: String,
    reader: i64,
    reader_email: String,
    role_id: i64,
}

fn request(
    site_id: i64,
    user_id: i64,
    page_reference: Option<Reference<'static>>,
) -> RequestContext {
    RequestContext {
        site_id: Some(site_id),
        user_id: Some(user_id),
        page_reference,
        ..Default::default()
    }
}

fn context<'a>(
    runner: &'a TestRunner,
    tx: &'a DatabaseTransaction,
    req: RequestContext,
) -> ServiceContext<'a> {
    ServiceContext::new(runner.state(), tx).with_request(req)
}

async fn fixture(runner: &TestRunner, tx: &DatabaseTransaction) -> Fixture {
    let suffix = Uuid::new_v4().simple().to_string();
    let ctx = context(runner, tx, RequestContext::default());
    let site_id = SiteService::create(
        &ctx,
        CreateSite {
            slug: format!("watch-delivery-{suffix}"),
            name: "Watching delivery".into(),
            tagline: String::new(),
            description: "Isolated watcher delivery fixture".into(),
            default_page: None,
            layout: None,
            license: License::CcBySa40,
            locale: "en".into(),
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .site_id;
    let role_id = RoleService::create(
        &ctx,
        InternalCreateRoleInput {
            site_id,
            name: "member".into(),
            description: None,
            is_virtual: true,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .role_id;
    let mut permissions = vec![Permission {
        resource_type: Resource::Site,
        resource_category: None,
        action: Action::View,
    }];
    permissions.extend([Action::View, Action::Create, Action::Edit].map(|action| {
        Permission {
            resource_type: Resource::Page,
            resource_category: None,
            action,
        }
    }));
    PermissionService::update_permissions_for_role(
        &ctx,
        UpdateRolePermissionsInput {
            site_id,
            role_reference: Reference::Id(role_id),
            new_permissions: permissions,
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    let author_name = format!("watch-author-{suffix}");
    let author = member(&ctx, site_id, &author_name).await;
    let reader_name = format!("watch-reader-{suffix}");
    let reader = member(&ctx, site_id, &reader_name).await;
    let reader_email = format!("{reader_name}@example.com");
    user::ActiveModel {
        user_id: Set(reader),
        email_verified_at: Set(Some(OffsetDateTime::now_utc())),
        ..Default::default()
    }
    .update(tx)
    .await
    .unwrap();
    Fixture {
        site_id,
        author,
        author_name,
        reader,
        reader_email,
        role_id,
    }
}

async fn member(ctx: &ServiceContext<'_>, site_id: i64, name: &str) -> i64 {
    let user_id = UserService::create(
        ctx,
        CreateUser {
            user_type: UserType::Regular,
            name: name.into(),
            email: format!("{name}@example.com"),
            locales: vec!["en".into()],
            password: "test-password".into(),
            bypass_filter: true,
            bypass_email_verification: true,
            override_user_id: None,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .user_id;
    RelationService::create_site_member(
        ctx,
        CreateSiteMember {
            site_id,
            user_id,
            created_by: SYSTEM_USER_ID,
            metadata: SiteMemberData {
                accepted: SiteMemberAccepted::SelfJoined,
            },
        },
        common::IP_ADDRESS,
    )
    .await
    .unwrap();
    user_id
}

async fn subscribe(
    runner: &TestRunner,
    tx: &DatabaseTransaction,
    f: &Fixture,
    scope: WatchScope,
    target: i64,
) {
    let ctx = context(runner, tx, request(f.site_id, f.reader, None));
    subscriptions::subscription_set(&ctx, f.site_id, scope, target, true)
        .await
        .unwrap();
}

async fn create(
    runner: &TestRunner,
    tx: &DatabaseTransaction,
    f: &Fixture,
    slug: &str,
    source: &str,
    quiet: bool,
) -> (i64, i64) {
    let ctx = context(
        runner,
        tx,
        request(
            f.site_id,
            f.author,
            Some(Reference::Slug(slug.to_owned().into())),
        ),
    );
    let created = deepwell::endpoints::all::page_create(&ctx, common::make_params(json!({
        "site_id": f.site_id, "user_id": f.author, "slug": slug, "title": "Watched story",
        "wikitext": source, "revision_comments": "created", "tags": [], "bypass_filter": true,
        "do_not_notify_watchers": quiet, "ip_address": common::IP_ADDRESS,
    }))).await.unwrap();
    (created.page_id, created.revision_id)
}

async fn edit(
    runner: &TestRunner,
    tx: &DatabaseTransaction,
    f: &Fixture,
    page: i64,
    previous: i64,
    source: &str,
    quiet: bool,
) -> Option<i64> {
    let ctx = context(
        runner,
        tx,
        request(f.site_id, f.author, Some(Reference::Id(page))),
    );
    deepwell::endpoints::all::page_edit(&ctx, common::make_params(json!({
        "site_id": f.site_id, "user_id": f.author, "page": page,
        "last_revision_id": previous, "wikitext": source, "revision_comments": "edited",
        "do_not_notify_watchers": quiet, "ip_address": common::IP_ADDRESS,
    }))).await.unwrap().map(|edited| edited.revision_id)
}

async fn event_id(runner: &TestRunner, revision: i64) -> Option<i64> {
    runner
        .state()
        .database
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT event_id FROM watch_event WHERE new_revision_id = $1",
            [revision.into()],
        ))
        .await
        .unwrap()
        .map(|row| row.try_get("", "event_id").unwrap())
}

async fn activity_for(
    runner: &TestRunner,
    f: &Fixture,
    before: Option<i64>,
    limit: u64,
) -> activity::ActivityPage {
    let tx = runner.state().database.begin().await.unwrap();
    let ctx = context(runner, &tx, request(f.site_id, f.reader, None));
    let page = activity::list_activity(&ctx, f.site_id, before, limit)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    page
}

async fn retire_site(runner: &TestRunner, f: &Fixture) {
    let tx = runner.state().database.begin().await.unwrap();
    site::ActiveModel {
        site_id: Set(f.site_id),
        deleted_at: Set(Some(OffsetDateTime::now_utc())),
        ..Default::default()
    }
    .update(&tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

async fn enable_email(runner: &TestRunner, tx: &DatabaseTransaction, f: &Fixture) {
    let ctx = context(runner, tx, request(f.site_id, f.reader, None));
    subscriptions::preferences_set(
        &ctx,
        WatchPreferences {
            email_enabled: true,
            auto_watch: false,
        },
    )
    .await
    .unwrap();
}

async fn delivery_state(
    runner: &TestRunner,
    event: i64,
    recipient: i64,
) -> (bool, bool, String, Option<String>) {
    let row = runner.state().database.query_one_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT processed_at IS NOT NULL AS processed, activity_ready, email_status, last_error FROM watch_notification WHERE event_id = $1 AND user_id = $2",
        [event.into(), recipient.into()],
    )).await.unwrap().unwrap();
    (
        row.try_get("", "processed").unwrap(),
        row.try_get("", "activity_ready").unwrap(),
        row.try_get("", "email_status").unwrap(),
        row.try_get("", "last_error").unwrap(),
    )
}

fn message(requests: &Arc<Mutex<Vec<String>>>, index: usize) -> String {
    let captured = requests.lock().unwrap();
    form_field(&captured[index], "text")
}

#[tokio::test]
async fn recorded_mailgun_failure_is_not_replayed_and_activity_remains_ready() {
    let (sender, requests) = fake_mailgun_with_status(500).await;
    let runner = TestRunner::setup_with_mailgun(sender).await;
    let tx = runner.state().database.begin().await.unwrap();
    let f = fixture(&runner, &tx).await;
    subscribe(&runner, &tx, &f, WatchScope::Site, f.site_id).await;
    enable_email(&runner, &tx, &f).await;
    let (_, revision) = create(
        &runner,
        &tx,
        &f,
        "story:failed-email",
        "Visible change.",
        false,
    )
    .await;
    tx.commit().await.unwrap();
    let event = event_id(&runner, revision).await.unwrap();

    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 1);
    assert_eq!(wait_for_requests(&requests, 1).await.len(), 1);
    assert_eq!(
        delivery_state(&runner, event, f.reader).await,
        (
            true,
            true,
            "failed".into(),
            Some("email attempt failed; delivery may be uncertain".into()),
        )
    );
    assert_eq!(
        activity_for(&runner, &f, None, 10).await.items[0].event_id,
        event
    );
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 0);
    assert_eq!(wait_for_requests(&requests, 1).await.len(), 1);
    retire_site(&runner, &f).await;
}

#[tokio::test]
async fn revoked_page_view_after_commit_prevents_both_delivery_channels() {
    let (sender, requests) = fake_mailgun().await;
    let runner = TestRunner::setup_with_mailgun(sender).await;
    let tx = runner.state().database.begin().await.unwrap();
    let f = fixture(&runner, &tx).await;
    subscribe(&runner, &tx, &f, WatchScope::Site, f.site_id).await;
    enable_email(&runner, &tx, &f).await;
    let (page, created) =
        create(&runner, &tx, &f, "story:revoked", "First version.", true).await;
    let edited = edit(&runner, &tx, &f, page, created, "Changed version.", false)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let event = event_id(&runner, edited).await.unwrap();

    let tx = runner.state().database.begin().await.unwrap();
    PermissionService::update_permissions_for_role(
        &context(&runner, &tx, RequestContext::default()),
        UpdateRolePermissionsInput {
            site_id: f.site_id,
            role_reference: Reference::Id(f.role_id),
            new_permissions: vec![Permission {
                resource_type: Resource::Site,
                resource_category: None,
                action: Action::View,
            }],
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 1);
    assert_eq!(
        delivery_state(&runner, event, f.reader).await,
        (true, false, "disabled".into(), None)
    );
    assert!(activity_for(&runner, &f, None, 10).await.items.is_empty());
    assert_eq!(wait_for_requests(&requests, 0).await.len(), 0);
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 0);
    retire_site(&runner, &f).await;
}

#[tokio::test]
async fn recursive_stored_revision_render_fails_closed_without_stopping_later_delivery() {
    let (sender, requests) = fake_mailgun().await;
    let runner = TestRunner::setup_with_mailgun(sender).await;
    let tx = runner.state().database.begin().await.unwrap();
    let f = fixture(&runner, &tx).await;
    let ctx = context(&runner, &tx, RequestContext::default());
    let everyone = RoleService::create(
        &ctx,
        InternalCreateRoleInput {
            site_id: f.site_id,
            name: "everyone".into(),
            description: None,
            is_virtual: true,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .role_id;
    PermissionService::update_permissions_for_role(
        &ctx,
        UpdateRolePermissionsInput {
            site_id: f.site_id,
            role_reference: Reference::Id(everyone),
            new_permissions: [Resource::Site, Resource::Page]
                .map(|resource_type| Permission {
                    resource_type,
                    resource_category: None,
                    action: Action::View,
                })
                .to_vec(),
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    subscribe(&runner, &tx, &f, WatchScope::Site, f.site_id).await;
    enable_email(&runner, &tx, &f).await;
    tx.commit().await.unwrap();

    for corrupt_old in [false, true] {
        let slug = if corrupt_old {
            "watch-broken-old"
        } else {
            "watch-broken-new"
        };
        let tx = runner.state().database.begin().await.unwrap();
        let helper_slug = format!("{slug}-cycle");
        let (_, helper_revision) =
            create(&runner, &tx, &f, &helper_slug, "Before.", true).await;
        let ctx = context(&runner, &tx, RequestContext::default());
        let helper_hash = TextService::create(&ctx, format!("[[include {helper_slug}]]"))
            .await
            .unwrap();
        page_revision::ActiveModel {
            revision_id: Set(helper_revision),
            wikitext_hash: Set(helper_hash.to_vec()),
            ..Default::default()
        }
        .update(&tx)
        .await
        .unwrap();
        let (page, created) =
            create(&runner, &tx, &f, slug, "Before.", corrupt_old).await;
        let changed = if corrupt_old {
            edit(&runner, &tx, &f, page, created, "After.", false)
                .await
                .unwrap()
        } else {
            created
        };
        let corrupted_revision = if corrupt_old { created } else { changed };
        let ctx = context(&runner, &tx, RequestContext::default());
        let hash = TextService::create(&ctx, format!("[[include {helper_slug}]]"))
            .await
            .unwrap();
        page_revision::ActiveModel {
            revision_id: Set(corrupted_revision),
            wikitext_hash: Set(hash.to_vec()),
            ..Default::default()
        }
        .update(&tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let event = event_id(&runner, changed).await.unwrap();
        let tx = runner.state().database.begin().await.unwrap();
        let ctx = context(&runner, &tx, request(f.site_id, f.reader, None));
        assert!(
            visible_change(
                &ctx,
                f.reader,
                f.site_id,
                page,
                corrupt_old.then_some(created),
                changed,
            )
            .await
            .is_err(),
            "stored revision must fail rendering before worker delivery"
        );
        tx.rollback().await.unwrap();

        assert_eq!(worker::process_one(runner.state()).await.unwrap(), 1);
        assert_eq!(
            delivery_state(&runner, event, f.reader).await,
            (
                true,
                false,
                "failed".into(),
                Some("revision visibility/rendering failed".into()),
            )
        );
        assert!(activity_for(&runner, &f, None, 10).await.items.is_empty());
        assert_eq!(wait_for_requests(&requests, 0).await.len(), 0);
    }

    let tx = runner.state().database.begin().await.unwrap();
    let (_, healthy) =
        create(&runner, &tx, &f, "story:healthy", "Safe content.", false).await;
    tx.commit().await.unwrap();
    let event = event_id(&runner, healthy).await.unwrap();
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 1);
    assert_eq!(delivery_state(&runner, event, f.reader).await.0, true);
    assert_eq!(
        activity_for(&runner, &f, None, 10).await.items[0].event_id,
        event
    );
    assert_eq!(wait_for_requests(&requests, 1).await.len(), 1);
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 0);
    retire_site(&runner, &f).await;
}

#[tokio::test]
async fn committed_changes_deliver_once_with_visible_bounded_diff_and_cursor() {
    let (sender, requests) = fake_mailgun().await;
    let runner = TestRunner::setup_with_mailgun(sender).await;
    let tx = runner.state().database.begin().await.unwrap();
    let f = fixture(&runner, &tx).await;
    subscribe(&runner, &tx, &f, WatchScope::Site, f.site_id).await;
    let ctx = context(&runner, &tx, request(f.site_id, f.reader, None));
    subscriptions::preferences_set(
        &ctx,
        WatchPreferences {
            email_enabled: true,
            auto_watch: false,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let tx = runner.state().database.begin().await.unwrap();
    let old = "An old color.\n[[include :snippets:suo BEGIN CODE |type=showto |user1=watch-author]]\nPrivate original.\n[[include :snippets:suo END CODE]]";
    let (page, created) = create(&runner, &tx, &f, "story:delivery", old, false).await;
    assert_eq!(
        worker::process_one(runner.state()).await.unwrap(),
        0,
        "uncommitted page cannot be delivered"
    );
    assert!(event_id(&runner, created).await.is_none());
    assert!(requests.lock().unwrap().is_empty());
    tx.commit().await.unwrap();
    let created_event = event_id(&runner, created).await.unwrap();
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 1);
    assert_eq!(wait_for_requests(&requests, 1).await.len(), 1);
    assert_eq!(activity_for(&runner, &f, None, 10).await.items.len(), 1);
    let tx = runner.state().database.begin().await.unwrap();
    let ctx = context(&runner, &tx, request(f.site_id, f.reader, None));
    let created_detail = activity::get_change(&ctx, created_event)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(created_detail.item.event_type, "create");
    assert_eq!(created_detail.before_text, None);
    assert!(created_detail.after_text.contains("An old color."));
    assert!(!created_detail.after_text.contains("Private original."));
    let page_record =
        deepwell::services::page::PageService::get(&ctx, f.site_id, Reference::Id(page))
            .await
            .unwrap();
    tx.rollback().await.unwrap();

    let tx = runner.state().database.begin().await.unwrap();
    subscribe(
        &runner,
        &tx,
        &f,
        WatchScope::Category,
        page_record.page_category_id,
    )
    .await;
    subscribe(&runner, &tx, &f, WatchScope::Page, page).await;
    let long_new = format!("A new color. {}", "界".repeat(1200));
    let edited = edit(&runner, &tx, &f, page, created, &long_new, false)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let edited_event = event_id(&runner, edited).await.unwrap();
    assert_eq!(
        worker::process_one(runner.state()).await.unwrap(),
        1,
        "three overlapping scopes create one recipient"
    );
    assert_eq!(wait_for_requests(&requests, 2).await.len(), 2);
    let email = message(&requests, 1);
    assert!(email.contains("Watched story") && email.contains(" edited this page at "));
    assert!(email.contains("- ") && email.contains("+ "));
    assert!(!email.contains("Private original.") && !email.contains("[[include"));
    let diff = email.split("\n\n").nth(1).unwrap();
    assert!(
        diff.chars().count() <= 1000,
        "rendered diff is bounded by Unicode characters"
    );
    assert!(email.contains(&format!("/-/activity?event={edited_event}")));
    assert!(email.contains("/-/watching-unsubscribe?token="));
    assert_eq!(
        form_field(&requests.lock().unwrap()[1], "to"),
        f.reader_email
    );
    let tx = runner.state().database.begin().await.unwrap();
    let reader = context(&runner, &tx, request(f.site_id, f.reader, None));
    let detail = activity::get_change(&reader, edited_event)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        detail.before_text.as_deref(),
        Some(created_detail.after_text.as_str())
    );
    assert!(detail.after_text.contains("A new color."));
    let visible_diff = deepwell::services::watching::diff::rendered_diff(
        detail.before_text.as_deref(),
        &detail.after_text,
    );
    assert!(
        email.contains(&visible_diff),
        "email describes the same recipient-visible change as Activity"
    );
    assert_eq!(detail.item.actor, f.author_name);
    tx.rollback().await.unwrap();

    let tx = runner.state().database.begin().await.unwrap();
    let third = edit(&runner, &tx, &f, page, edited, "A third color.", false)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 1);
    let first = activity_for(&runner, &f, None, 1).await;
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].revision_id, third);
    let second = activity_for(&runner, &f, first.next_before, 1).await;
    assert_eq!(second.items[0].revision_id, edited);
    let third_page = activity_for(&runner, &f, second.next_before, 1).await;
    assert_eq!(third_page.items[0].revision_id, created);
    assert_eq!(third_page.next_before, None);
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 0);
    assert_eq!(wait_for_requests(&requests, 3).await.len(), 3);
    let tx = runner.state().database.begin().await.unwrap();
    let self_ctx = context(&runner, &tx, request(f.site_id, f.author, None));
    assert!(
        activity::list_activity(&self_ctx, f.site_id, None, 10)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    tx.rollback().await.unwrap();
    retire_site(&runner, &f).await;
}

#[tokio::test]
async fn unsubscribe_disables_email_but_keeps_watches_and_future_activity() {
    let (sender, requests) = fake_mailgun().await;
    let runner = TestRunner::setup_with_mailgun(sender).await;
    let tx = runner.state().database.begin().await.unwrap();
    let f = fixture(&runner, &tx).await;
    subscribe(&runner, &tx, &f, WatchScope::Site, f.site_id).await;
    let ctx = context(&runner, &tx, request(f.site_id, f.reader, None));
    subscriptions::preferences_set(
        &ctx,
        WatchPreferences {
            email_enabled: true,
            auto_watch: false,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let tx = runner.state().database.begin().await.unwrap();
    let (page, created) =
        create(&runner, &tx, &f, "story:optout", "First page.", false).await;
    tx.commit().await.unwrap();
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 1);
    assert_eq!(wait_for_requests(&requests, 1).await.len(), 1);
    let email = message(&requests, 0);
    let token = email
        .split("/-/watching-unsubscribe?token=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    let tx = runner.state().database.begin().await.unwrap();
    let anonymous = context(&runner, &tx, RequestContext::default());
    assert!(
        activity::unsubscribe_email(&anonymous, token)
            .await
            .unwrap()
    );
    assert!(
        !activity::unsubscribe_email(&anonymous, token)
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();
    let tx = runner.state().database.begin().await.unwrap();
    let changed = edit(&runner, &tx, &f, page, created, "Second page.", false)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 1);
    assert_eq!(wait_for_requests(&requests, 1).await.len(), 1);
    let tx = runner.state().database.begin().await.unwrap();
    let reader = context(&runner, &tx, request(f.site_id, f.reader, None));
    assert!(
        !subscriptions::preferences_get(&reader)
            .await
            .unwrap()
            .email_enabled
    );
    assert_eq!(
        subscriptions::subscriptions(&reader, f.site_id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        activity::list_activity(&reader, f.site_id, None, 10)
            .await
            .unwrap()
            .items[0]
            .revision_id,
        changed
    );
    tx.rollback().await.unwrap();
    retire_site(&runner, &f).await;
}

#[tokio::test]
async fn hidden_previous_revision_and_revoked_read_fail_closed_at_delivery_and_readback()
{
    let (sender, requests) = fake_mailgun().await;
    let runner = TestRunner::setup_with_mailgun(sender).await;
    let tx = runner.state().database.begin().await.unwrap();
    let f = fixture(&runner, &tx).await;
    subscribe(&runner, &tx, &f, WatchScope::Site, f.site_id).await;
    let ctx = context(&runner, &tx, request(f.site_id, f.reader, None));
    subscriptions::preferences_set(
        &ctx,
        WatchPreferences {
            email_enabled: true,
            auto_watch: false,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let tx = runner.state().database.begin().await.unwrap();
    let (page, created) = create(
        &runner,
        &tx,
        &f,
        "story:privacy",
        "Old sensitive body.",
        true,
    )
    .await;
    let edited = edit(
        &runner,
        &tx,
        &f,
        page,
        created,
        "New sensitive body.",
        false,
    )
    .await
    .unwrap();
    page_revision::ActiveModel {
        revision_id: Set(created),
        hidden: Set(vec!["wikitext".into()]),
        ..Default::default()
    }
    .update(&tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let event = event_id(&runner, edited).await.unwrap();
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 1);
    assert_eq!(wait_for_requests(&requests, 0).await.len(), 0);
    let tx = runner.state().database.begin().await.unwrap();
    let reader = context(&runner, &tx, request(f.site_id, f.reader, None));
    assert!(
        activity::list_activity(&reader, f.site_id, None, 10)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        activity::get_change(&reader, event)
            .await
            .unwrap()
            .is_none()
    );
    tx.rollback().await.unwrap();

    let tx = runner.state().database.begin().await.unwrap();
    page_revision::ActiveModel {
        revision_id: Set(created),
        hidden: Set(vec![]),
        ..Default::default()
    }
    .update(&tx)
    .await
    .unwrap();
    let visible = edit(
        &runner,
        &tx,
        &f,
        page,
        edited,
        "Latest sensitive body.",
        false,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let visible_event = event_id(&runner, visible).await.unwrap();
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 1);
    let tx = runner.state().database.begin().await.unwrap();
    PermissionService::update_permissions_for_role(
        &context(&runner, &tx, RequestContext::default()),
        UpdateRolePermissionsInput {
            site_id: f.site_id,
            role_reference: Reference::Id(f.role_id),
            new_permissions: vec![Permission {
                resource_type: Resource::Page,
                resource_category: None,
                action: Action::Edit,
            }],
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let tx = runner.state().database.begin().await.unwrap();
    let reader = context(&runner, &tx, request(f.site_id, f.reader, None));
    assert!(
        activity::get_change(&reader, visible_event)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        activity::list_activity(&reader, f.site_id, None, 10)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    tx.rollback().await.unwrap();
    retire_site(&runner, &f).await;
}

#[tokio::test]
async fn suppression_rollback_and_import_leave_no_delivery_but_future_edit_does() {
    let (sender, requests) = fake_mailgun().await;
    let runner = TestRunner::setup_with_mailgun(sender).await;
    let tx = runner.state().database.begin().await.unwrap();
    let f = fixture(&runner, &tx).await;
    subscribe(&runner, &tx, &f, WatchScope::Site, f.site_id).await;
    tx.commit().await.unwrap();

    let tx = runner.state().database.begin().await.unwrap();
    let (quiet_page, quiet_created) =
        create(&runner, &tx, &f, "story:quiet", "Quiet create.", true).await;
    let quiet_edited = edit(
        &runner,
        &tx,
        &f,
        quiet_page,
        quiet_created,
        "Quiet edit.",
        true,
    )
    .await
    .unwrap();
    let no_change = edit(
        &runner,
        &tx,
        &f,
        quiet_page,
        quiet_edited,
        "Quiet edit.",
        false,
    )
    .await;
    assert!(
        no_change.is_none(),
        "unchanged edit must not produce a revision"
    );
    tx.commit().await.unwrap();
    assert!(event_id(&runner, quiet_created).await.is_none());
    assert!(event_id(&runner, quiet_edited).await.is_none());

    let tx = runner.state().database.begin().await.unwrap();
    let (rolled_page, rolled_revision) =
        create(&runner, &tx, &f, "story:rollback", "Not committed.", false).await;
    tx.rollback().await.unwrap();
    assert!(event_id(&runner, rolled_revision).await.is_none());
    let tx = runner.state().database.begin().await.unwrap();
    let row = tx
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT count(*) AS total FROM page WHERE site_id=$1 AND page_id=$2",
            [f.site_id.into(), rolled_page.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "total").unwrap(), 0);
    tx.rollback().await.unwrap();

    let tx = runner.state().database.begin().await.unwrap();
    let ctx = context(
        &runner,
        &tx,
        request(
            f.site_id,
            f.author,
            Some(Reference::Slug("story:imported".into())),
        ),
    );
    let imported = deepwell::endpoints::all::page_import(
        &ctx,
        common::make_params(json!({
            "site_id": f.site_id, "user_id": f.author, "slug": "story:imported",
            "title": "Imported story", "wikitext": "Historical text.",
            "revision_comments": "imported", "tags": [], "bypass_filter": true,
            "ip_address": common::IP_ADDRESS,
        })),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert!(event_id(&runner, imported.revision_id).await.is_none());
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 0);
    assert_eq!(wait_for_requests(&requests, 0).await.len(), 0);
    assert!(activity_for(&runner, &f, None, 10).await.items.is_empty());

    let tx = runner.state().database.begin().await.unwrap();
    let future = edit(
        &runner,
        &tx,
        &f,
        imported.page_id,
        imported.revision_id,
        "Future local text.",
        false,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let future_event = event_id(&runner, future).await.unwrap();
    assert_eq!(worker::process_one(runner.state()).await.unwrap(), 1);
    assert_eq!(
        activity_for(&runner, &f, None, 10).await.items[0].event_id,
        future_event
    );
    assert_eq!(
        wait_for_requests(&requests, 0).await.len(),
        0,
        "watcher email defaults off"
    );
    retire_site(&runner, &f).await;
}
