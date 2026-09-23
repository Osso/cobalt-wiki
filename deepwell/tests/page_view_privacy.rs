mod common;

use common::TestRunner;
use deepwell::constants::SYSTEM_USER_ID;
use deepwell::license::License;
use deepwell::models::session;
use deepwell::services::ServiceContext;
use deepwell::services::category::CategoryService;
use deepwell::services::page::{CreatePage, PageService};
use deepwell::services::permission::PermissionService;
use deepwell::services::relation::{
    CreatePageAttribution, CreateSiteMember, PageAttributionKind,
    PageAttributionMetadata, RelationService, SiteMemberAccepted, SiteMemberData,
};
use deepwell::services::role::{
    InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::site::{CreateSite, SiteService};
use deepwell::services::user::{CreateUser, UserService};
use deepwell::services::view::GetPageViewOutput;
use deepwell::types::{Action, Permission, Reference, Resource, UserType};
use sea_orm::{ActiveModelTrait, Set};
use time::{Duration, OffsetDateTime};

async fn member(ctx: &ServiceContext<'_>, site_id: i64, label: &str) -> (i64, String) {
    let user = UserService::create(
        ctx,
        CreateUser {
            user_type: UserType::Regular,
            name: format!("Privacy {label}"),
            email: format!("privacy-{label}@example.com"),
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
    // Integration config generates 16-byte tokens, below the database's minimum.
    // Seed a valid session to test page authorization, not token generation.
    let token = format!(
        "wj:privacy-view-{label}-{}-{}",
        user.user_id,
        "x".repeat(48)
    );
    let now = OffsetDateTime::now_utc();
    session::ActiveModel {
        session_token: Set(token.clone()),
        user_id: Set(user.user_id),
        created_at: Set(now),
        expires_at: Set(now + Duration::hours(1)),
        ip_address: Set(common::IP_ADDRESS.to_string()),
        user_agent: Set("privacy-test".into()),
        restricted: Set(false),
    }
    .insert(ctx.transaction())
    .await
    .unwrap();
    (user.user_id, token)
}

async fn page(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    user_id: i64,
    slug: &str,
    source: &str,
) {
    let created = PageService::import(
        ctx,
        CreatePage {
            site_id,
            user_id,
            slug: slug.into(),
            title: slug.into(),
            alt_title: None,
            wikitext: source.into(),
            layout: None,
            revision_comments: "fixture".into(),
            tags: vec![],
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    RelationService::create_page_attribution(
        ctx,
        CreatePageAttribution {
            page_id: created.page_id,
            user_id,
            created_by: SYSTEM_USER_ID,
            metadata: PageAttributionMetadata {
                attribution_type: PageAttributionKind::Author,
                attribution_date: OffsetDateTime::now_utc().date(),
            },
        },
    )
    .await
    .unwrap();
}

async fn view(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    token: Option<&str>,
    slug: &str,
) -> GetPageViewOutput {
    deepwell::endpoints::view::page_view(
        ctx,
        common::make_params(serde_json::json!({
            "site_id": site_id, "session_token": token, "locales": ["en"],
            "route": {"slug": slug, "extra": ""},
        })),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn attributed_page_and_template_privacy() {
    let runner = TestRunner::setup().await;
    let ctx = runner.context();
    let site_id = SiteService::create(
        ctx,
        CreateSite {
            slug: "privacy-context".into(),
            name: "Privacy context".into(),
            tagline: String::new(),
            description: "Privacy test fixture".into(),
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
    let (creator, creator_token) = member(ctx, site_id, "creator").await;
    let (other, other_token) = member(ctx, site_id, "other").await;
    for (role, category) in [("page-author", "application"), ("member", "player")] {
        let category_id = CategoryService::get_or_create(ctx, site_id, category)
            .await
            .unwrap()
            .category_id;
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
        PermissionService::update_permissions_for_role(
            ctx,
            UpdateRolePermissionsInput {
                site_id,
                role_reference: Reference::Id(role_id),
                new_permissions: vec![Permission {
                    resource_type: Resource::Page,
                    resource_category: Some(Reference::Id(category_id)),
                    action: Action::View,
                }],
                cascade_removals: false,
                updating_user_id: SYSTEM_USER_ID,
                ip_address: common::IP_ADDRESS,
            },
        )
        .await
        .unwrap();
    }
    page(ctx, site_id, creator, "application:own", "name: Secret\n").await;
    page(
        ctx,
        site_id,
        creator,
        "application:_template",
        "[[form]]\nfields:\n  name:\n    type: text\n[[/form]]",
    )
    .await;
    page(ctx, site_id, other, "application:other", "name: Other\n").await;
    page(ctx, site_id, creator, "player:profile", "Member profile").await;

    let own = view(ctx, site_id, Some(&creator_token), "application:own").await;
    match own {
        GetPageViewOutput::Found { wikitext, form, .. } => {
            assert_eq!(wikitext, "name: Secret\n");
            assert_eq!(form.unwrap()["values"]["name"], "Secret");
        }
        _ => panic!("attributed creator must view own page and template"),
    }
    match view(ctx, site_id, Some(&other_token), "application:other").await {
        GetPageViewOutput::Found { form, .. } => assert!(
            form.is_none(),
            "page ownership must not disclose another author's template"
        ),
        _ => panic!("other creator must view own page"),
    }
    for token in [None, Some(other_token.as_str())] {
        assert!(matches!(
            view(ctx, site_id, token, "application:own").await,
            GetPageViewOutput::Permissions { .. }
        ));
    }
    assert!(matches!(
        view(ctx, site_id, Some(&creator_token), "application:other").await,
        GetPageViewOutput::Permissions { .. }
    ));
    for (token, slug, expected_source) in [
        (creator_token.as_str(), "application:own", "name: Secret\n"),
        (other_token.as_str(), "application:other", "name: Other\n"),
    ] {
        match view(ctx, site_id, Some(token), slug).await {
            GetPageViewOutput::Found { wikitext, .. } => {
                assert_eq!(wikitext, expected_source)
            }
            _ => panic!("denial for another page must not deny the creator's own page"),
        }
        assert!(matches!(
            view(ctx, site_id, None, slug).await,
            GetPageViewOutput::Permissions { .. }
        ));
    }
    for token in [creator_token.as_str(), other_token.as_str()] {
        assert!(matches!(
            view(ctx, site_id, Some(token), "player:profile").await,
            GetPageViewOutput::Found { .. }
        ));
        assert!(matches!(
            view(ctx, site_id, Some(token), "application:missing").await,
            GetPageViewOutput::Missing { .. }
        ));
    }
    assert!(matches!(
        view(ctx, site_id, None, "player:profile").await,
        GetPageViewOutput::Permissions { .. }
    ));
    assert!(matches!(
        view(ctx, site_id, None, "application:missing").await,
        GetPageViewOutput::Missing { .. }
    ));
}
