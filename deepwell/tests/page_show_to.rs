//! Wikidot `:snippets:suo` show-to regions reach only the users they list: the
//! stored HTML omits them, and a listed signed-in viewer gets them rendered.

mod common;

use common::TestRunner;
use deepwell::constants::SYSTEM_USER_ID;
use deepwell::models::{session, site};
use deepwell::services::ServiceContext;
use deepwell::services::page::{CreatePage, PageService};
use deepwell::services::relation::{
    CreateSiteMember, RelationService, SiteMemberAccepted, SiteMemberData,
};
use deepwell::services::site::SiteService;
use deepwell::services::user::{CreateUser, UserService};
use deepwell::services::view::GetPageViewOutput;
use deepwell::types::{Reference, UserType};
use sea_orm::{ActiveModelTrait, Set};
use time::{Duration, OffsetDateTime};

const NAV_TOP: &str = "* [[[start|Home]]]\n\n[[include :snippets:suo BEGIN CODE |type=showto | user1=ozmaasimov | user2=Show-Admin ]]\n* **[# Admin]**\n * [[[_admin|Site Manager]]]\n[[include :snippets:suo END CODE]]\n\n* [[[roster|Profiles]]]";

const START: &str = "Welcome.\n\n[[include :snippets:suo BEGIN CODE |type=showto |user1=show-admin]]\nAdmin checklist\n[[include :snippets:suo END CODE]]\n\n[[include :snippets:suo BEGIN CODE |type=showto |user1=someone-else]]\nOther secret\n[[include :snippets:suo END CODE]]\n\n[[include show-to-included]]";

/// Regions are revealed only in the rendered page's own source, never in included pages.
const INCLUDED: &str = "Included text.\n[[include :snippets:suo BEGIN CODE |type=showto |user1=show-admin]]\nIncluded secret\n[[include :snippets:suo END CODE]]";

/// A site member with a session; returns the session token.
async fn member(ctx: &ServiceContext<'_>, site_id: i64, name: &str) -> String {
    let user = UserService::create(
        ctx,
        CreateUser {
            user_type: UserType::Regular,
            name: name.into(),
            email: format!("{}@example.com", name.replace(' ', "-")),
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
    let token = format!("wj:show-to-{}-{}", user.user_id, "x".repeat(48));
    let now = OffsetDateTime::now_utc();
    session::ActiveModel {
        session_token: Set(token.clone()),
        user_id: Set(user.user_id),
        created_at: Set(now),
        expires_at: Set(now + Duration::hours(1)),
        ip_address: Set(common::IP_ADDRESS.to_string()),
        user_agent: Set("show-to-test".into()),
        restricted: Set(false),
    }
    .insert(ctx.transaction())
    .await
    .unwrap();
    token
}

async fn page(ctx: &ServiceContext<'_>, site_id: i64, slug: &str, source: &str) {
    PageService::import(
        ctx,
        CreatePage {
            site_id,
            user_id: SYSTEM_USER_ID,
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
}

/// Body and top bar HTML of a view.
async fn view(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    token: Option<&str>,
    slug: &str,
) -> (String, String) {
    let output = deepwell::endpoints::view::page_view(
        ctx,
        common::make_params(serde_json::json!({
            "site_id": site_id, "session_token": token, "locales": ["en"],
            "route": {"slug": slug, "extra": ""},
        })),
    )
    .await
    .unwrap();
    match output {
        GetPageViewOutput::Found {
            compiled_body_html,
            compiled_top_bar_html,
            ..
        }
        | GetPageViewOutput::Missing {
            compiled_body_html,
            compiled_top_bar_html,
            ..
        } => (compiled_body_html, compiled_top_bar_html.expect("top bar")),
        other => panic!("{slug} must be found or missing: {other:?}"),
    }
}

#[tokio::test]
async fn show_to_regions_render_only_for_listed_viewers() {
    let runner = TestRunner::setup().await;
    let ctx = runner.context();
    let site_id = SiteService::get(ctx, Reference::Slug("test".into()))
        .await
        .unwrap()
        .site_id;
    site::ActiveModel {
        site_id: Set(site_id),
        top_bar_page: Set("show-to-top".into()),
        ..Default::default()
    }
    .update(ctx.transaction())
    .await
    .unwrap();
    let admin = member(ctx, site_id, "Show Admin").await;
    let other = member(ctx, site_id, "Show Member").await;
    page(ctx, site_id, "show-to-top", NAV_TOP).await;
    page(ctx, site_id, "show-to-included", INCLUDED).await;
    page(ctx, site_id, "show-to-start", START).await;

    let restricted = [
        "Site Manager",
        "Admin checklist",
        "Other secret",
        "Included secret",
        "suo",
    ];
    let assert_hidden = |(body, top): &(String, String), viewer: &str| {
        for text in restricted {
            assert!(!body.contains(text), "{viewer} body has {text:?}: {body}");
            assert!(!top.contains(text), "{viewer} top bar has {text:?}: {top}");
        }
        assert!(
            !body.contains("Included page unavailable"),
            "{viewer}: {body}"
        );
        assert!(top.contains("Profiles"), "{viewer} top bar: {top}");
    };

    let anonymous = view(ctx, site_id, None, "show-to-start").await;
    assert_hidden(&anonymous, "anonymous");
    let unlisted = view(ctx, site_id, Some(&other), "show-to-start").await;
    assert_hidden(&unlisted, "unlisted member");
    assert_eq!(
        unlisted, anonymous,
        "an unlisted member sees the shared HTML"
    );

    let (body, top) = view(ctx, site_id, Some(&admin), "show-to-start").await;
    assert!(
        body.contains("Welcome.") && body.contains("Admin checklist"),
        "{body}"
    );
    assert!(body.contains("Included text."), "{body}");
    assert!(!body.contains("Other secret"), "{body}");
    assert!(!body.contains("Included secret"), "{body}");
    assert!(
        top.contains("Site Manager") && top.contains("Profiles"),
        "{top}"
    );
    assert!(!body.contains("suo") && !top.contains("suo"));

    // The listed viewer's render is not stored: others still get the shared HTML.
    assert_eq!(view(ctx, site_id, None, "show-to-start").await, anonymous);
    assert_eq!(
        view(ctx, site_id, Some(&other), "show-to-start").await,
        anonymous
    );

    // A missing page shows the site navigation, likewise per viewer.
    let (_, top) = view(ctx, site_id, Some(&admin), "show-to-missing").await;
    assert!(top.contains("Site Manager"), "{top}");
    for token in [None, Some(other.as_str())] {
        assert_hidden(
            &view(ctx, site_id, token, "show-to-missing").await,
            "missing page",
        );
    }
}
