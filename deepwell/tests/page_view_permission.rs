//! `page_view_permission`: WWS's Page/View check before serving a page's
//! files and text blocks, for the session's viewer and for anonymous caching.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::{ADMIN_USER_ID, SYSTEM_USER_ID};
use deepwell::license::License;
use deepwell::models::session;
use deepwell::services::ServiceContext;
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
use deepwell::types::{Action, Permission, Reference, Resource, UserType};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use time::{Duration, OffsetDateTime};

async fn member_session(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    label: &str,
) -> (i64, String) {
    let user = UserService::create(
        ctx,
        CreateUser {
            user_type: UserType::Regular,
            name: format!("File viewer {label}"),
            email: format!("file-viewer-{label}@example.com"),
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
    // Integration config generates tokens below the database's minimum length.
    let token = format!("wj:file-view-{label}-{}-{}", user.user_id, "x".repeat(48));
    let now = OffsetDateTime::now_utc();
    session::ActiveModel {
        session_token: Set(token.clone()),
        user_id: Set(user.user_id),
        created_at: Set(now),
        expires_at: Set(now + Duration::hours(1)),
        ip_address: Set(common::IP_ADDRESS.to_string()),
        user_agent: Set("file-view-test".into()),
        restricted: Set(false),
    }
    .insert(ctx.transaction())
    .await
    .unwrap();
    (user.user_id, token)
}

async fn grant_view(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    role: &str,
    categories: &[&str],
) {
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
    let mut new_permissions = Vec::new();
    for category in categories {
        let category_id = CategoryService::get_or_create(ctx, site_id, category)
            .await
            .unwrap()
            .category_id;
        new_permissions.push(Permission {
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
            new_permissions,
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
}

async fn page_id(ctx: &ServiceContext<'_>, site_id: i64, slug: &str) -> i64 {
    PageService::import(
        ctx,
        CreatePage {
            site_id,
            user_id: SYSTEM_USER_ID,
            slug: slug.into(),
            title: slug.into(),
            alt_title: None,
            wikitext: "[[file chessset.jpg]]".into(),
            layout: None,
            revision_comments: "fixture".into(),
            tags: vec![],
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .page_id
}

async fn check(
    runner: &TestRunner,
    site_id: i64,
    page_id: i64,
    token: Option<&str>,
) -> (bool, bool) {
    let output = run_endpoint!(
        runner,
        page_view_permission,
        json!({"site_id": site_id, "page_id": page_id, "session_token": token}),
    );
    (output.can_view, output.public)
}

#[tokio::test]
async fn viewer_and_anonymous_page_view_permission() {
    let runner = TestRunner::setup().await;
    let ctx = runner.context();
    let site_id = SiteService::create(
        ctx,
        CreateSite {
            slug: "file-view-permission".into(),
            name: "File view permission".into(),
            tagline: String::new(),
            description: "File permission fixture".into(),
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
    // Anonymous visitors see `writing`; members also see `admin`.
    // Banned users get neither (no `anonymous`/`member` virtual role).
    grant_view(ctx, site_id, "anonymous", &["writing"]).await;
    grant_view(ctx, site_id, "member", &["writing", "admin"]).await;
    grant_view(ctx, site_id, "banned", &[]).await;
    let public_page = page_id(
        ctx,
        site_id,
        "writing:2021-10-21-to-paint-a-picture:the-game",
    )
    .await;
    let private_page = page_id(ctx, site_id, "admin:css").await;
    let (_, member_token) = member_session(ctx, site_id, "member").await;
    let (banned_id, banned_token) = member_session(ctx, site_id, "banned").await;
    run_endpoint!(
        runner,
        site_ban_set,
        json!({
            "site_id": site_id,
            "user_id": banned_id,
            "metadata": {"banned_until": null, "reason": "fixture"},
            "created_by": ADMIN_USER_ID,
            "ip_address": common::IP_ADDRESS,
        }),
    );
    let unknown_token = format!("wj:file-view-unknown-{}", "x".repeat(48));

    // Public page: everyone but the banned user; cacheable publicly.
    assert_eq!(
        check(&runner, site_id, public_page, None).await,
        (true, true)
    );
    assert_eq!(
        check(&runner, site_id, public_page, Some(&member_token)).await,
        (true, true),
    );
    assert_eq!(
        check(&runner, site_id, public_page, Some(&banned_token)).await,
        (false, true),
    );

    // Private page: only the member; never public.
    assert_eq!(
        check(&runner, site_id, private_page, None).await,
        (false, false)
    );
    assert_eq!(
        check(&runner, site_id, private_page, Some(&member_token)).await,
        (true, false),
    );
    assert_eq!(
        check(&runner, site_id, private_page, Some(&banned_token)).await,
        (false, false),
    );

    // An unknown or expired cookie views as anonymous, like the page view.
    assert_eq!(
        check(&runner, site_id, private_page, Some(&unknown_token)).await,
        (false, false),
    );
    assert_eq!(
        check(&runner, site_id, public_page, Some(&unknown_token)).await,
        (true, true),
    );

    // A page ID from another site is not checked against this site's roles.
    run_endpoint_err!(
        runner,
        page_view_permission,
        json!({"site_id": site_id + 99999, "page_id": private_page, "session_token": member_token}),
    );
}
