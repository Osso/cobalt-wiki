//! The page-facing backlinks API exposes only currently viewable source pages.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::{ADMIN_USER_ID, SYSTEM_USER_ID};
use deepwell::license::License;
use deepwell::models::{page, page_connection, page_revision};
use deepwell::services::RequestContext;
use deepwell::services::category::CategoryService;
use deepwell::services::page::{CreatePage, PageService};
use deepwell::services::permission::PermissionService;
use deepwell::services::relation::{
    CreateSiteMember, RelationService, SiteMemberAccepted, SiteMemberData,
};
use deepwell::services::role::{
    InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::site::{CreateSite, SiteService};
use deepwell::services::user::{CreateUser, UserService};
use deepwell::types::{
    Action, ConnectionType, Permission, Reference, Resource, UserType,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{Value, json};

async fn create_site(runner: &TestRunner, slug: &str) -> i64 {
    SiteService::create(
        runner.context(),
        CreateSite {
            slug: slug.into(),
            name: slug.into(),
            tagline: String::new(),
            description: slug.into(),
            default_page: None,
            layout: None,
            license: License::CcBySa40,
            locale: "en".into(),
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .site_id
}

async fn grant_view(runner: &TestRunner, site_id: i64, role: &str, categories: &[&str]) {
    let ctx = runner.context();
    let role_id = RoleService::create(
        ctx,
        InternalCreateRoleInput {
            site_id,
            name: role.into(),
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
    let mut permissions = Vec::new();
    for category in categories {
        let category_id = CategoryService::get_or_create(ctx, site_id, category)
            .await
            .unwrap()
            .category_id;
        permissions.push(Permission {
            resource_type: Resource::Page,
            resource_category: Some(Reference::Id(category_id)),
            action: Action::View,
        });
    }
    PermissionService::update_permissions_for_role(
        ctx,
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
}

async fn import_page(runner: &TestRunner, site_id: i64, slug: &str, title: &str) -> i64 {
    PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            slug: slug.into(),
            title: title.into(),
            wikitext: "fixture".into(),
            alt_title: None,
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "fixture".into(),
            tags: vec![],
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .page_id
}

async fn connect(
    runner: &TestRunner,
    from: i64,
    to: i64,
    connection_type: ConnectionType,
) {
    page_connection::ActiveModel {
        from_page_id: Set(from),
        to_page_id: Set(to),
        connection_type: Set(connection_type),
        count: Set(3),
        ..Default::default()
    }
    .insert(runner.context().transaction())
    .await
    .unwrap();
}

fn backlinks(
    runner: &TestRunner,
    site_id: i64,
    page_id: i64,
) -> impl std::future::Future<Output = Value> + '_ {
    async move {
        serde_json::to_value(run_endpoint!(
            runner,
            page_backlinks,
            json!({"site_id": site_id, "page_id": page_id})
        ))
        .unwrap()
    }
}

#[tokio::test]
async fn backlinks_show_only_live_same_site_viewable_sources_with_current_titles() {
    let runner = TestRunner::setup().await;
    let site_id = create_site(&runner, "backlinks-sources").await;
    let other_site = create_site(&runner, "backlinks-foreign").await;
    grant_view(&runner, site_id, "anonymous", &["public"]).await;
    let target = import_page(&runner, site_id, "public:target", "Target").await;
    let z = import_page(&runner, site_id, "public:z", "Old title").await;
    let a = import_page(&runner, site_id, "public:a", "A title").await;
    let hidden = import_page(&runner, site_id, "private:hidden", "Hidden title").await;
    let deleted = import_page(&runner, site_id, "public:deleted", "Deleted title").await;
    let foreign = import_page(&runner, other_site, "foreign", "Foreign title").await;
    page::ActiveModel {
        page_id: Set(deleted),
        deleted_at: Set(Some(time::OffsetDateTime::now_utc())),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
    let latest = PageService::get(runner.context(), site_id, Reference::Id(z))
        .await
        .unwrap()
        .latest_revision_id
        .unwrap();
    page_revision::ActiveModel {
        revision_id: Set(latest),
        title: Set("Current title".into()),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
    for (from, kind) in [
        (z, ConnectionType::Link),
        (a, ConnectionType::Link),
        (z, ConnectionType::IncludeMessy),
        (z, ConnectionType::IncludeElements),
        (a, ConnectionType::IncludeElements),
        (hidden, ConnectionType::Link),
        (hidden, ConnectionType::IncludeMessy),
        (deleted, ConnectionType::Link),
        (foreign, ConnectionType::Link),
        (a, ConnectionType::Redirect),
        (a, ConnectionType::Component),
    ] {
        connect(&runner, from, target, kind).await;
    }
    let result = backlinks(&runner, site_id, target).await;
    assert_eq!(
        result,
        json!({
            "links": [
                {"page_id": a, "slug": "public:a", "title": "A title"},
                {"page_id": z, "slug": "public:z", "title": "Current title"}
            ],
            "inclusions": [
                {"page_id": a, "slug": "public:a", "title": "A title"},
                {"page_id": z, "slug": "public:z", "title": "Current title"}
            ]
        })
    );
    let before = page_revision::Entity::find()
        .filter(page_revision::Column::PageId.eq(target))
        .all(runner.context().transaction())
        .await
        .unwrap();
    assert_eq!(backlinks(&runner, site_id, target).await, result);
    let after = page_revision::Entity::find()
        .filter(page_revision::Column::PageId.eq(target))
        .all(runner.context().transaction())
        .await
        .unwrap();
    assert_eq!(
        before, after,
        "backlinks read must not create or change revisions"
    );
}

#[tokio::test]
async fn member_sees_private_sources_but_banned_member_cannot_view_public_target() {
    let mut runner = TestRunner::setup().await;
    let site_id = create_site(&runner, "backlinks-member").await;
    grant_view(&runner, site_id, "anonymous", &["public"]).await;
    grant_view(&runner, site_id, "member", &["public", "private"]).await;
    grant_view(&runner, site_id, "banned", &[]).await;
    let target = import_page(&runner, site_id, "public:target", "Target").await;
    let private = import_page(&runner, site_id, "private:source", "Member source").await;
    connect(&runner, private, target, ConnectionType::Link).await;
    assert_eq!(
        backlinks(&runner, site_id, target).await,
        json!({"links": [], "inclusions": []})
    );

    let user_id = UserService::create(
        runner.context(),
        CreateUser {
            user_type: UserType::Regular,
            name: "Backlink member".into(),
            email: "backlink-member@example.com".into(),
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
        runner.context(),
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
    runner.set_request_context(RequestContext {
        user_id: Some(user_id),
        site_id: Some(site_id),
        ..Default::default()
    });
    assert_eq!(
        backlinks(&runner, site_id, target).await,
        json!({
            "links": [{"page_id": private, "slug": "private:source", "title": "Member source"}],
            "inclusions": []
        })
    );
    run_endpoint!(
        runner,
        site_ban_set,
        json!({
            "site_id": site_id,
            "user_id": user_id,
            "metadata": {"banned_until": null, "reason": "fixture"},
            "created_by": ADMIN_USER_ID,
            "ip_address": common::IP_ADDRESS,
        })
    );
    run_endpoint_err!(
        runner,
        page_backlinks,
        json!({"site_id": site_id, "page_id": target})
    );
}

#[tokio::test]
async fn denied_missing_deleted_and_foreign_targets_never_return_backlinks() {
    let mut runner = TestRunner::setup().await;
    let site_id = create_site(&runner, "backlinks-target").await;
    let other_site = create_site(&runner, "backlinks-other-target").await;
    grant_view(&runner, site_id, "anonymous", &["public"]).await;
    let private = import_page(&runner, site_id, "private:target", "Private").await;
    let deleted = import_page(&runner, site_id, "public:deleted-target", "Deleted").await;
    let foreign = import_page(&runner, other_site, "foreign-target", "Foreign").await;
    page::ActiveModel {
        page_id: Set(deleted),
        deleted_at: Set(Some(time::OffsetDateTime::now_utc())),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
    for page_id in [private, deleted, foreign, i64::MAX] {
        run_endpoint_err!(
            runner,
            page_backlinks,
            json!({"site_id": site_id, "page_id": page_id})
        );
    }
    runner.set_request_context(RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        ..Default::default()
    });
    // A valid authenticated context is not authority to read a different site's target.
    run_endpoint_err!(
        runner,
        page_backlinks,
        json!({"site_id": site_id, "page_id": foreign})
    );
}
