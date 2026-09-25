mod common;

use common::TestRunner;
use deepwell::constants::SYSTEM_USER_ID;
use deepwell::license::License;
use deepwell::services::RequestContext;
use deepwell::services::page::PageService;
use deepwell::services::permission::PermissionService;
use deepwell::services::relation::{
    CreateSiteMember, RelationService, SiteMemberAccepted, SiteMemberData,
};
use deepwell::services::role::{
    InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::site::{CreateSite, SiteService};
use deepwell::services::user::{CreateUser, UserService};
use deepwell::services::watching::subscriptions::{self, WatchScope};
use deepwell::types::{Action, Permission, Reference, Resource, UserType};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::json;

async fn make_site(runner: &TestRunner) -> i64 {
    let ctx = runner.context();
    let site = SiteService::create(
        ctx,
        CreateSite {
            slug: "watch-event-site".into(),
            name: "Watch events".into(),
            tagline: String::new(),
            description: "Watcher event fixtures".into(),
            default_page: None,
            layout: None,
            license: License::CcBySa40,
            locale: "en".into(),
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    let role = RoleService::create(
        ctx,
        InternalCreateRoleInput {
            site_id: site.site_id,
            name: "member".into(),
            description: None,
            is_virtual: true,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
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
        ctx,
        UpdateRolePermissionsInput {
            site_id: site.site_id,
            role_reference: Reference::Id(role.role_id),
            new_permissions: permissions,
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    site.site_id
}

async fn make_member(runner: &TestRunner, site_id: i64, name: &str) -> i64 {
    let ctx = runner.context();
    let user = UserService::create(
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
    .unwrap();
    RelationService::create_site_member(
        ctx,
        CreateSiteMember {
            site_id,
            user_id: user.user_id,
            created_by: SYSTEM_USER_ID,
            metadata: SiteMemberData {
                accepted: SiteMemberAccepted::SelfJoined,
            },
        },
        common::IP_ADDRESS,
    )
    .await
    .unwrap();
    user.user_id
}

fn set_actor(runner: &mut TestRunner, site_id: i64, user_id: i64) {
    runner.set_request_context(RequestContext {
        user_id: Some(user_id),
        site_id: Some(site_id),
        ..Default::default()
    });
}

async fn recipient_ids(runner: &TestRunner, revision_id: i64) -> Vec<i64> {
    runner.context().transaction().query_all_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT n.user_id FROM watch_notification n JOIN watch_event e USING(event_id) WHERE e.new_revision_id=$1 ORDER BY n.user_id",
        [revision_id.into()],
    )).await.unwrap().iter().map(|row| row.try_get("", "user_id").unwrap()).collect()
}

#[tokio::test]
async fn ordinary_page_changes_capture_scopes_without_self_import_or_suppressed_notifications()
 {
    let mut runner = TestRunner::setup().await;
    let site_id = make_site(&runner).await;
    let author = make_member(&runner, site_id, "watch-event-author").await;
    let subscriber = make_member(&runner, site_id, "watch-event-reader").await;
    for user_id in [author, subscriber] {
        set_actor(&mut runner, site_id, user_id);
        subscriptions::subscription_set(
            runner.context(),
            site_id,
            WatchScope::Site,
            site_id,
            true,
        )
        .await
        .unwrap();
    }
    set_actor(&mut runner, site_id, author);
    runner.set_request_context(RequestContext {
        user_id: Some(author),
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug("story:watched".into())),
        ..Default::default()
    });
    let created = deepwell::endpoints::all::page_create(runner.context(), common::make_params(json!({
        "site_id":site_id,"user_id":author,"slug":"story:watched","title":"Watched",
        "wikitext":"Initial visible text","revision_comments":"created","tags":[],"bypass_filter":true,"ip_address":common::IP_ADDRESS,
    }))).await.unwrap();
    assert_eq!(
        recipient_ids(&runner, created.revision_id).await,
        vec![subscriber]
    );
    let observer = runner.context().state().database.clone();
    let row = observer
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT count(*) AS count FROM watch_event WHERE new_revision_id=$1",
            [created.revision_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        row.try_get::<i64>("", "count").unwrap(),
        0,
        "uncommitted change must not reach a delivery observer"
    );

    let page =
        PageService::get(runner.context(), site_id, Reference::Id(created.page_id))
            .await
            .unwrap();
    set_actor(&mut runner, site_id, subscriber);
    subscriptions::subscription_set(
        runner.context(),
        site_id,
        WatchScope::Category,
        page.page_category_id,
        true,
    )
    .await
    .unwrap();
    subscriptions::subscription_set(
        runner.context(),
        site_id,
        WatchScope::Page,
        page.page_id,
        true,
    )
    .await
    .unwrap();
    set_actor(&mut runner, site_id, author);
    runner.set_request_context(RequestContext {
        user_id: Some(author),
        site_id: Some(site_id),
        page_reference: Some(Reference::Id(created.page_id)),
        ..Default::default()
    });
    let edited = deepwell::endpoints::all::page_edit(runner.context(), common::make_params(json!({
        "site_id":site_id,"user_id":author,"page":created.page_id,"last_revision_id":created.revision_id,
        "wikitext":"Changed visible text","revision_comments":"changed","ip_address":common::IP_ADDRESS,
    }))).await.unwrap().unwrap();
    assert_eq!(
        recipient_ids(&runner, edited.revision_id).await,
        vec![subscriber],
        "overlapping watches notify once"
    );
    let suppressed = deepwell::endpoints::all::page_edit(runner.context(), common::make_params(json!({
        "site_id":site_id,"user_id":author,"page":created.page_id,"last_revision_id":edited.revision_id,
        "wikitext":"Quiet visible text","revision_comments":"quiet","do_not_notify_watchers":true,"ip_address":common::IP_ADDRESS,
    }))).await.unwrap().unwrap();
    assert!(
        recipient_ids(&runner, suppressed.revision_id)
            .await
            .is_empty()
    );
    assert_ne!(
        suppressed.revision_id, edited.revision_id,
        "suppression preserves the revision"
    );
    let imported = deepwell::endpoints::all::page_import(runner.context(), common::make_params(json!({
        "site_id":site_id,"user_id":author,"slug":"story:imported","title":"Imported",
        "wikitext":"Historical source","revision_comments":"imported","tags":[],"bypass_filter":true,"ip_address":common::IP_ADDRESS,
    }))).await.unwrap();
    assert!(
        recipient_ids(&runner, imported.revision_id)
            .await
            .is_empty()
    );
    runner.set_request_context(RequestContext {
        user_id: Some(author),
        site_id: Some(site_id),
        page_reference: Some(Reference::Id(imported.page_id)),
        ..Default::default()
    });
    let future_edit = deepwell::endpoints::all::page_edit(runner.context(), common::make_params(json!({
        "site_id":site_id,"user_id":author,"page":imported.page_id,"last_revision_id":imported.revision_id,
        "wikitext":"New local change","revision_comments":"future change","ip_address":common::IP_ADDRESS,
    }))).await.unwrap().unwrap();
    assert_eq!(
        recipient_ids(&runner, future_edit.revision_id).await,
        vec![subscriber]
    );
}
