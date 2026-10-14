//! Join display uses native site membership, not authentication, and never
//! personalizes shared compiled HTML.
mod common;

use common::TestRunner;
use deepwell::constants::SYSTEM_USER_ID;
use deepwell::models::{session, site};
use deepwell::services::ServiceContext;
use deepwell::services::TextService;
use deepwell::services::page::{CreatePage, PageService};
use deepwell::services::relation::{
    CreateSiteMember, RelationService, RemoveSiteMember, SiteMemberAccepted,
    SiteMemberData,
};
use deepwell::services::site::SiteService;
use deepwell::services::user::{CreateUser, UserService};
use deepwell::services::view::GetPageViewOutput;
use deepwell::types::{Reference, UserType};
use scraper::{Html, Selector};
use sea_orm::{ActiveModelTrait, Set};
use time::{Duration, OffsetDateTime};

const SOURCE: &str = "Welcome.\n\n[[module Join button=\"Join this wiki!\"]]\n\n[[module Join button=\"Apply <now>\"]]\n\n[[[/-/join|Membership information]]]\n\nStill here.";

async fn account(ctx: &ServiceContext<'_>, name: &str) -> i64 {
    UserService::create(
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
    .user_id
}

async fn session_token(ctx: &ServiceContext<'_>, user_id: i64) -> String {
    // Integration token settings are shorter than the DB minimum.
    let token = format!("wj:join-view-{user_id}-{}", "x".repeat(48));
    let now = OffsetDateTime::now_utc();
    session::ActiveModel {
        session_token: Set(token.clone()),
        user_id: Set(user_id),
        created_at: Set(now),
        expires_at: Set(now + Duration::hours(1)),
        ip_address: Set(common::IP_ADDRESS.to_string()),
        user_agent: Set("join-test".into()),
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

async fn page(ctx: &ServiceContext<'_>, site_id: i64, slug: &str) {
    PageService::import(
        ctx,
        CreatePage {
            site_id,
            user_id: SYSTEM_USER_ID,
            slug: slug.into(),
            title: slug.into(),
            alt_title: None,
            wikitext: SOURCE.into(),
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

async fn view(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    token: Option<&str>,
) -> GetPageViewOutput {
    deepwell::endpoints::view::page_view(
        ctx,
        common::make_params(serde_json::json!({
            "site_id": site_id, "session_token": token, "locales": ["en"],
            "route": null,
        })),
    )
    .await
    .unwrap()
}

fn assert_fragment(html: &str, visible: bool) {
    let dom = Html::parse_fragment(html);
    let joins = Selector::parse(".join-box a[href='/-/join']").unwrap();
    let labels: Vec<_> = dom
        .select(&joins)
        .map(|a| a.text().collect::<String>())
        .collect();
    let expected = if visible {
        vec!["Join this wiki!", "Apply <now>"]
    } else {
        vec![]
    };
    assert_eq!(labels, expected, "{html}");
    let text = dom.root_element().text().collect::<String>();
    for preserved in ["Welcome.", "Still here.", "Membership information"] {
        assert!(text.contains(preserved), "{html}");
    }
    let link = Selector::parse("a[href='/-/join']").unwrap();
    assert_eq!(dom.select(&link).count(), if visible { 3 } else { 1 });
}

async fn assert_view(ctx: &ServiceContext<'_>, output: GetPageViewOutput, visible: bool) {
    let GetPageViewOutput::Found {
        wikitext,
        page_revision,
        compiled_body_html,
        compiled_top_bar_html,
        compiled_side_bar_html,
        ..
    } = output
    else {
        panic!("expected homepage")
    };
    assert_eq!(wikitext, SOURCE, "page source must not be personalized");
    assert_fragment(&compiled_body_html, visible);
    assert_fragment(&compiled_top_bar_html.expect("top bar"), visible);
    assert_fragment(&compiled_side_bar_html.expect("side bar"), visible);
    for hash in [
        Some(page_revision.compiled_body_html_hash),
        page_revision.compiled_top_bar_html_hash,
        page_revision.compiled_side_bar_html_hash,
    ]
    .into_iter()
    .flatten()
    {
        let stored = TextService::get(ctx, &hash).await.unwrap();
        assert_fragment(&stored, true);
    }
}

#[tokio::test]
async fn join_cta_is_per_membership_without_contaminating_shared_renders() {
    let runner = TestRunner::setup_with_idle_job_workers().await;
    let ctx = runner.context();
    let site_id = SiteService::get(ctx, Reference::Slug("test".into()))
        .await
        .unwrap()
        .site_id;
    let member_id = account(ctx, "JoinMember").await;
    let guest_id = account(ctx, "JoinGuest").await;
    join(ctx, site_id, member_id).await;
    let member = session_token(ctx, member_id).await;
    let guest = session_token(ctx, guest_id).await;
    site::ActiveModel {
        site_id: Set(site_id),
        default_page: Set("join-start".into()),
        top_bar_page: Set("join-top".into()),
        side_bar_page: Set("join-side".into()),
        ..Default::default()
    }
    .update(ctx.transaction())
    .await
    .unwrap();
    for slug in ["join-top", "join-side", "join-start"] {
        page(ctx, site_id, slug).await;
    }
    // Member first, then guests, then member again against the same compiled hashes.
    for (token, visible) in [
        (Some(member.as_str()), false),
        (None, true),
        (Some(guest.as_str()), true),
        (Some(member.as_str()), false),
        (Some(guest.as_str()), true),
        (None, true),
    ] {
        assert_view(ctx, view(ctx, site_id, token).await, visible).await;
    }
    // Authentication remains unchanged while native membership changes.
    join(ctx, site_id, guest_id).await;
    assert_view(ctx, view(ctx, site_id, Some(&guest)).await, false).await;
    RelationService::remove_site_member(
        ctx,
        RemoveSiteMember {
            site_id,
            user_id: guest_id,
            removed_by: SYSTEM_USER_ID,
        },
        common::IP_ADDRESS,
        "test membership transition",
    )
    .await
    .unwrap();
    assert_view(ctx, view(ctx, site_id, Some(&guest)).await, true).await;
    assert_view(ctx, view(ctx, site_id, Some(&member)).await, false).await;
    assert_view(ctx, view(ctx, site_id, None).await, true).await;
}
