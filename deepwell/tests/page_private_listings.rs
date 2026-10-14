//! Native session rendering must reveal private form/list data only to eligible viewers.
mod common;

use common::TestRunner;
use deepwell::constants::SYSTEM_USER_ID;
use deepwell::license::License;
use deepwell::models::session;
use deepwell::services::category::CategoryService;
use deepwell::services::page::{CreatePage, PageService};
use deepwell::services::permission::PermissionService;
use deepwell::services::relation::{
    CreatePageAttribution, CreateSiteMember, PageAttributionKind,
    PageAttributionMetadata, RelationService, SiteMemberAccepted, SiteMemberData,
};
use deepwell::services::role::{
    GrantUserRoleInput, InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::site::{CreateSite, SiteService};
use deepwell::services::user::{CreateUser, UserService};
use deepwell::services::view::GetPageViewOutput;
use deepwell::services::{PageRevisionService, ServiceContext, TextService};
use deepwell::types::{Action, Permission, Reference, Resource, UserType};
use scraper::{Html, Selector};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{Value, json};
use time::{Duration, OffsetDateTime};

const PORTRAIT: &str = "/-/file/player:orchid/private-player-portrait.png";
const RECORD: &str =
    "name: Private Player Orchid\nportrait: private-player-portrait.png\n";
const PLAYER_BOX: &str =
    "[[image {$image} style=\"margin: 0; border: 0;\" width=\"320px\" alt=\"\"]]";
const TEMPLATE: &str = "Profile: **%%form_data{name}%%**\n\n[[include player:_biography]]\n\n[[include public:playerbox\n| image=%%fullname%%/%%form_raw{portrait}%%\n]]\n\n====\n\n[[form]]\nfields:\n  name:\n    type: text\n  portrait:\n    type: text\n[[/form]]\n";
const LISTING: &str = "Profile directory\n\n[[module ListPages category=\"player\" order=\"name\" separate=\"no\"]]\n%%linked_title%%: %%form_data{name}%%\n\n[[include public:playerbox\n| image=%%fullname%%/%%form_raw{portrait}%%\n]]\n[[/module]]\n\nDirectory end";

struct Fixture {
    site_id: i64,
    eligible: Vec<(String, String)>,
    registered: String,
}

struct SharedPage {
    source: String,
    revision: Value,
    html: String,
}

async fn account(ctx: &ServiceContext<'_>, label: &str) -> i64 {
    UserService::create(
        ctx,
        CreateUser {
            user_type: UserType::Regular,
            name: format!("Listings {label}"),
            email: format!("listings-{label}@example.com"),
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
    .user_id
}

async fn token(ctx: &ServiceContext<'_>, user_id: i64) -> String {
    let token = format!("wj:private-listings-{user_id}-{}", "x".repeat(48));
    let now = OffsetDateTime::now_utc();
    session::ActiveModel {
        session_token: Set(token.clone()),
        user_id: Set(user_id),
        created_at: Set(now),
        expires_at: Set(now + Duration::hours(1)),
        ip_address: Set(common::IP_ADDRESS.to_string()),
        user_agent: Set("private-listings-test".into()),
        restricted: Set(false),
    }
    .insert(ctx.transaction())
    .await
    .unwrap();
    token
}

async fn join(ctx: &ServiceContext<'_>, site_id: i64, user_id: i64) {
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
}

async fn role(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    name: &str,
    parent: Option<i64>,
) -> i64 {
    RoleService::create(
        ctx,
        InternalCreateRoleInput {
            site_id,
            name: name.into(),
            description: None,
            is_virtual: matches!(name, "everyone" | "member"),
            parent_role_id: parent,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .role_id
}

async fn allow(ctx: &ServiceContext<'_>, site_id: i64, role_id: i64, categories: &[i64]) {
    PermissionService::update_permissions_for_role(
        ctx,
        UpdateRolePermissionsInput {
            site_id,
            role_reference: Reference::Id(role_id),
            new_permissions: categories
                .iter()
                .map(|id| Permission {
                    resource_type: Resource::Page,
                    resource_category: Some(Reference::Id(*id)),
                    action: Action::View,
                })
                .collect(),
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
}

async fn assign(ctx: &ServiceContext<'_>, site_id: i64, user_id: i64, role_id: i64) {
    RoleService::grant_role_to_user(
        ctx,
        GrantUserRoleInput {
            site_id,
            user_id,
            role_id,
            assigning_user_id: SYSTEM_USER_ID,
            expires_at: None,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
}

async fn import(ctx: &ServiceContext<'_>, site_id: i64, slug: &str, source: &str) {
    PageService::import(
        ctx,
        CreatePage {
            site_id,
            user_id: SYSTEM_USER_ID,
            slug: slug.into(),
            title: slug.into(),
            alt_title: None,
            wikitext: source.into(),
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Private listings fixture".into(),
            tags: vec![],
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
}

async fn fixture(ctx: &ServiceContext<'_>, label: &str) -> Fixture {
    let site_id = SiteService::create(
        ctx,
        CreateSite {
            slug: format!("private-listings-{label}"),
            name: format!("Private listings {label}"),
            tagline: String::new(),
            description: "Rollback-only native privacy fixture".into(),
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
    let mut categories = Vec::new();
    for name in ["public", "player", "membersonly"] {
        categories.push(
            CategoryService::get_or_create(ctx, site_id, name)
                .await
                .unwrap()
                .category_id,
        );
    }
    let everyone = role(ctx, site_id, "everyone", None).await;
    allow(ctx, site_id, everyone, &categories[..1]).await;
    let member = role(ctx, site_id, "member", None).await;
    allow(ctx, site_id, member, &categories[1..]).await;
    let user_id = account(ctx, &format!("{label}-member")).await;
    join(ctx, site_id, user_id).await;
    let mut eligible = vec![("member".into(), token(ctx, user_id).await)];
    let mut parent = None;
    // These users are not native members: explicit rank grants must suffice.
    for name in ["root", "admin", "mod"] {
        let rank = role(ctx, site_id, name, parent).await;
        allow(ctx, site_id, rank, &categories[1..]).await;
        let user_id = account(ctx, &format!("{label}-{name}")).await;
        assign(ctx, site_id, user_id, rank).await;
        eligible.push((name.into(), token(ctx, user_id).await));
        parent = Some(rank);
    }
    let registered_id = account(ctx, &format!("{label}-registered")).await;
    let registered = token(ctx, registered_id).await;
    for (slug, source) in [
        ("public:playerbox", PLAYER_BOX),
        ("player:_biography", "Private included biography"),
        ("player:_template", TEMPLATE),
        ("player:orchid", RECORD),
        ("public:profileindex", LISTING),
        ("membersonly:profileindex", LISTING),
    ] {
        import(ctx, site_id, slug, source).await;
    }
    Fixture {
        site_id,
        eligible,
        registered,
    }
}

async fn shared(ctx: &ServiceContext<'_>, site_id: i64, slug: &str) -> SharedPage {
    let page = PageService::get(ctx, site_id, Reference::Slug(slug.into()))
        .await
        .unwrap();
    let revision = PageRevisionService::get_latest(ctx, site_id, page.page_id)
        .await
        .unwrap();
    SharedPage {
        source: TextService::get(ctx, &revision.wikitext_hash)
            .await
            .unwrap(),
        html: TextService::get(ctx, &revision.compiled_body_html_hash)
            .await
            .unwrap(),
        revision: serde_json::to_value(revision).unwrap(),
    }
}

async fn view(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    slug: &str,
    token: Option<&str>,
) -> GetPageViewOutput {
    deepwell::endpoints::view::page_view(
        ctx,
        common::make_params(json!({
            "site_id": site_id, "session_token": token, "locales": ["en"],
            "route": {"slug": slug, "extra": ""},
        })),
    )
    .await
    .unwrap()
}

fn found(output: GetPageViewOutput, original: &SharedPage) -> (String, Option<Value>) {
    let GetPageViewOutput::Found {
        wikitext,
        page_revision,
        compiled_body_html,
        form,
        ..
    } = output
    else {
        panic!("expected authorized page, got {output:?}")
    };
    assert_eq!(
        wikitext, original.source,
        "response must preserve raw source"
    );
    assert_eq!(
        serde_json::to_value(page_revision).unwrap(),
        original.revision,
        "personalized render must preserve revision and all compiled hashes"
    );
    (compiled_body_html, form)
}

fn assert_portrait(html: &str, visible: bool) {
    let dom = Html::parse_fragment(html);
    let images = Selector::parse("img").unwrap();
    let portraits: Vec<_> = dom
        .select(&images)
        .filter(|image| image.value().attr("src") == Some(PORTRAIT))
        .collect();
    assert_eq!(
        portraits.len(),
        usize::from(visible),
        "portrait visibility: {html}"
    );
    for portrait in portraits {
        assert_eq!(
            portrait.value().attr("crossorigin"),
            None,
            "portrait source requests no CORS; preserve browser image request mode: {html}"
        );
    }
    let text = dom.root_element().text().collect::<String>();
    assert_eq!(
        text.contains("Private Player Orchid"),
        visible,
        "private name visibility: {html}"
    );
    if !visible {
        assert!(
            !html.contains("player:orchid")
                && !html.contains(PORTRAIT)
                && !html.contains("Private included biography"),
            "private link/URL leaked: {html}"
        );
    }
}

async fn assert_unchanged(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    slug: &str,
    before: &SharedPage,
) {
    let after = shared(ctx, site_id, slug).await;
    assert_eq!(after.source, before.source);
    assert_eq!(after.revision, before.revision);
    assert_eq!(
        after.html, before.html,
        "session HTML must never overwrite shared text"
    );
}

async fn assert_media_permission(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    token: Option<&str>,
    allowed: bool,
) {
    let page = PageService::get(ctx, site_id, Reference::Slug("player:orchid".into()))
        .await
        .unwrap();
    let permission = deepwell::endpoints::view::page_view_permission(
        ctx,
        common::make_params(json!({
            "site_id": site_id, "page_id": page.page_id, "session_token": token,
        })),
    )
    .await
    .unwrap();
    assert_eq!(permission.can_view, allowed, "native media authorization");
    assert!(
        !permission.public,
        "private player media cannot be publicly cached"
    );
}

#[tokio::test]
async fn list_pages_page_author_fallback_is_response_only_for_attributed_member() {
    let runner = TestRunner::setup_with_idle_job_workers().await;
    let ctx = runner.context();
    let fixture = fixture(ctx, "author-fallback").await;
    let site_id = fixture.site_id;
    let category_id = CategoryService::get_or_create(ctx, site_id, "authored")
        .await
        .unwrap()
        .category_id;
    let page_author = RoleService::create(
        ctx,
        InternalCreateRoleInput {
            site_id,
            name: "page-author".into(),
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
    // No category-wide role grants View on authored pages.
    allow(ctx, site_id, page_author, &[category_id]).await;
    let author_id = account(ctx, "fallback-author").await;
    let other_id = account(ctx, "fallback-other").await;
    join(ctx, site_id, author_id).await;
    join(ctx, site_id, other_id).await;
    let author_token = token(ctx, author_id).await;
    let other_token = token(ctx, other_id).await;
    let page_slug = "authored:orchid";
    import(ctx, site_id, page_slug, "Author-only orchid").await;
    let page = PageService::get(ctx, site_id, Reference::Slug(page_slug.into()))
        .await
        .unwrap();
    RelationService::create_page_attribution(
        ctx,
        CreatePageAttribution {
            page_id: page.page_id,
            user_id: author_id,
            created_by: SYSTEM_USER_ID,
            metadata: PageAttributionMetadata {
                attribution_type: PageAttributionKind::Author,
                attribution_date: OffsetDateTime::now_utc().date(),
            },
        },
    )
    .await
    .unwrap();
    let index_slug = "public:authorindex";
    import(
        ctx,
        site_id,
        index_slug,
        "Author directory\n\n[[module ListPages category=\"authored\" separate=\"no\"]]\n%%linked_title%%\n[[/module]]\n\nDirectory end",
    )
    .await;
    let original = shared(ctx, site_id, index_slug).await;
    assert!(
        !original.html.contains(page_slug),
        "author-only page leaked into stored shared HTML: {}",
        original.html
    );
    let link = Selector::parse("a[href=\"/authored:orchid\"]").unwrap();
    // Alternate viewers to detect personalized output contaminating shared state.
    for (viewer_token, visible) in [
        (&author_token, true),
        (&other_token, false),
        (&author_token, true),
    ] {
        let (html, _) = found(
            view(ctx, site_id, index_slug, Some(viewer_token)).await,
            &original,
        );
        let dom = Html::parse_fragment(&html);
        assert_eq!(
            dom.select(&link).count(),
            usize::from(visible),
            "attributed member listing visibility: {html}"
        );
        assert!(
            html.contains("Author directory") && html.contains("Directory end"),
            "public index wrapper vanished: {html}"
        );
        if !visible {
            assert!(
                !html.contains(page_slug),
                "author-only page leaked to another member: {html}"
            );
        }
        assert_unchanged(ctx, site_id, index_slug, &original).await;
    }
}

#[tokio::test]
async fn private_profile_expands_form_and_portrait_only_for_member_or_higher() {
    let runner = TestRunner::setup_with_idle_job_workers().await;
    let ctx = runner.context();
    let fixture = fixture(ctx, "profile").await;
    let original = shared(ctx, fixture.site_id, "player:orchid").await;
    let template = shared(ctx, fixture.site_id, "player:_template").await;
    assert_eq!(original.source, RECORD);
    assert_eq!(template.source, TEMPLATE);
    for (rank, token) in &fixture.eligible {
        for guest in [None, Some(fixture.registered.as_str())] {
            assert_media_permission(ctx, fixture.site_id, Some(token), true).await;
            let (html, form) = found(
                view(ctx, fixture.site_id, "player:orchid", Some(token)).await,
                &original,
            );
            assert_eq!(
                form.unwrap()["values"]["portrait"],
                "private-player-portrait.png",
                "{rank}"
            );
            assert_portrait(&html, true);
            assert!(
                html.contains("Profile:") && html.contains("Private included biography"),
                "private template/include did not expand for {rank}: {html}"
            );
            assert!(
                !html.contains("name:") && !html.contains("%%form_data"),
                "raw form leaked: {html}"
            );
            assert_media_permission(ctx, fixture.site_id, guest, false).await;
            let denied = view(ctx, fixture.site_id, "player:orchid", guest).await;
            let GetPageViewOutput::Permissions {
                compiled_body_html, ..
            } = denied
            else {
                panic!("guest/nonmember saw private profile: {denied:?}")
            };
            assert_portrait(&compiled_body_html, false);
            assert!(matches!(
                view(ctx, fixture.site_id, "player:_template", guest).await,
                GetPageViewOutput::Permissions { .. }
            ));
            let (html, _) = found(
                view(ctx, fixture.site_id, "player:orchid", Some(token)).await,
                &original,
            );
            assert_portrait(&html, true);
            assert_unchanged(ctx, fixture.site_id, "player:orchid", &original).await;
            assert_unchanged(ctx, fixture.site_id, "player:_template", &template).await;
        }
    }
}

#[tokio::test]
async fn public_and_private_indexes_filter_private_profiles_per_session_without_shared_cache_leaks()
 {
    let runner = TestRunner::setup_with_idle_job_workers().await;
    let ctx = runner.context();
    let fixture = fixture(ctx, "index").await;
    let public = shared(ctx, fixture.site_id, "public:profileindex").await;
    let private = shared(ctx, fixture.site_id, "membersonly:profileindex").await;
    assert_eq!(public.source, LISTING);
    assert_eq!(private.source, LISTING);
    assert_portrait(&public.html, false);
    for (rank, token) in &fixture.eligible {
        for guest in [None, Some(fixture.registered.as_str())] {
            for slug in ["public:profileindex", "membersonly:profileindex"] {
                let original = if slug.starts_with("public:") {
                    &public
                } else {
                    &private
                };
                let (html, _) = found(
                    view(ctx, fixture.site_id, slug, Some(token)).await,
                    original,
                );
                assert_portrait(&html, true);
                assert!(
                    html.contains("player:orchid"),
                    "missing profile link for {rank}: {html}"
                );
            }
            let (html, _) = found(
                view(ctx, fixture.site_id, "public:profileindex", guest).await,
                &public,
            );
            assert_portrait(&html, false);
            assert!(
                html.contains("Profile directory") && html.contains("Directory end"),
                "public wrapper vanished: {html}"
            );
            let denied =
                view(ctx, fixture.site_id, "membersonly:profileindex", guest).await;
            let GetPageViewOutput::Permissions {
                compiled_body_html, ..
            } = denied
            else {
                panic!("guest/nonmember saw private index: {denied:?}")
            };
            assert_portrait(&compiled_body_html, false);
            let (html, _) = found(
                view(ctx, fixture.site_id, "public:profileindex", Some(token)).await,
                &public,
            );
            assert_portrait(&html, true);
            assert_unchanged(ctx, fixture.site_id, "public:profileindex", &public).await;
            assert_unchanged(ctx, fixture.site_id, "membersonly:profileindex", &private)
                .await;
        }
    }
}
