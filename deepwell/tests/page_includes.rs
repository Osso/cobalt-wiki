//! Shared rendered pages expand readable local includes without changing source.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::{page, page_revision, role_permission, site};
use deepwell::services::page::CreatePage;
use deepwell::services::page_revision::RerenderType;
use deepwell::services::{LinkService, PageRevisionService, PageService, TextService};
use deepwell::types::{Action, ConnectionType, PageId, Reference, Resource};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;

async fn import_page(runner: &TestRunner, site_id: i64, slug: &str, source: &str) {
    PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            slug: slug.into(),
            title: slug.into(),
            wikitext: source.into(),
            alt_title: None,
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Include rendering fixture".into(),
            tags: vec![],
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("import include fixture");
}

#[tokio::test]
async fn nested_local_includes_substitute_arguments_and_preserve_source() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    import_page(
        &runner,
        site_id,
        "include-fixture-inner",
        "Inner **{$word}**.",
    )
    .await;
    import_page(
        &runner,
        site_id,
        "include-fixture-outer",
        "Outer\n\n[[include include-fixture-inner | word={$word}]]",
    )
    .await;
    let source = "Before\n\n[[include include-fixture-outer | word=world]]\n\nAfter";
    import_page(&runner, site_id, "include-fixture-page", source).await;
    let page = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": "include-fixture-page", "details": {"wikitext": true, "compiled": true}})
    )
    .unwrap();
    assert_eq!(page.wikitext.as_deref(), Some(source));
    let html = page.compiled_body_html.unwrap();
    assert!(html.contains("Inner <strong>world</strong>"), "{html}");
    assert!(html.contains("Before") && html.contains("Outer") && html.contains("After"));
    assert!(!html.contains("[[include"), "{html}");
}

#[tokio::test]
async fn missing_deleted_and_foreign_includes_do_not_reveal_target_source() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    import_page(&runner, site_id, "include-deleted", "DELETED_CONTENT").await;
    import_page(
        &runner,
        site_id,
        "include-foreign",
        "LOCAL_COLLISION_CONTENT",
    )
    .await;
    let foreign_id = run_endpoint!(runner, site_get, json!({"site": "scp-wiki"}))
        .unwrap()
        .site
        .site_id;
    import_page(&runner, foreign_id, "include-foreign", "FOREIGN_CONTENT").await;
    let deleted = PageService::get(
        runner.context(),
        site_id,
        Reference::Slug("include-deleted".into()),
    )
    .await
    .unwrap();
    page::ActiveModel {
        page_id: Set(deleted.page_id),
        deleted_at: Set(Some(time::OffsetDateTime::now_utc())),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
    let source = "[[include include-missing]]\n\n[[include include-deleted]]\n\n[[include :scp-wiki:include-foreign]]";
    import_page(&runner, site_id, "include-unavailable", source).await;
    let page = run_endpoint!(runner, page_get, json!({"site_id":site_id,"page":"include-unavailable","details":{"compiled":true}})).unwrap();
    let html = page.compiled_body_html.unwrap();
    assert_eq!(
        html.matches("Included page unavailable.").count(),
        3,
        "{html}"
    );
    let foreign = PageService::get(
        runner.context(),
        foreign_id,
        Reference::Slug("include-foreign".into()),
    )
    .await
    .unwrap();
    let dependencies = LinkService::get_to(
        runner.context(),
        foreign.page_id,
        Some(&[ConnectionType::IncludeMessy]),
    )
    .await
    .unwrap();
    assert!(
        dependencies.connections.is_empty(),
        "unexpanded foreign content must not create an include dependency"
    );
    for forbidden in [
        "DELETED_CONTENT",
        "LOCAL_COLLISION_CONTENT",
        "FOREIGN_CONTENT",
    ] {
        assert!(!html.contains(forbidden), "{html}");
    }
}

#[tokio::test]
async fn shared_compilation_does_not_include_a_target_denied_to_anonymous_readers() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site":"test"}))
        .unwrap()
        .site
        .site_id;
    import_page(&runner, site_id, "include-restricted", "RESTRICTED_CONTENT").await;
    role_permission::Entity::delete_many()
        .filter(role_permission::Column::SiteId.eq(site_id))
        .filter(role_permission::Column::ResourceType.eq(Resource::Page))
        .filter(role_permission::Column::Action.eq(Action::View))
        .exec(runner.context().transaction())
        .await
        .unwrap();
    import_page(
        &runner,
        site_id,
        "include-denied-parent",
        "[[include include-restricted]]",
    )
    .await;
    let page = run_endpoint!(runner, page_get, json!({"site_id":site_id,"page":"include-denied-parent","details":{"compiled":true}})).unwrap();
    let html = page.compiled_body_html.unwrap();
    assert!(html.contains("Included page unavailable."), "{html}");
    assert!(!html.contains("RESTRICTED_CONTENT"), "{html}");
}

#[tokio::test]
async fn both_navigation_regions_expand_local_includes() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site":"test"}))
        .unwrap()
        .site
        .site_id;
    import_page(
        &runner,
        site_id,
        "include-nav-part",
        "**{$region} navigation**",
    )
    .await;
    import_page(
        &runner,
        site_id,
        "include-top",
        "[[include include-nav-part | region=Top]]",
    )
    .await;
    import_page(
        &runner,
        site_id,
        "include-side",
        "[[include include-nav-part | region=Side]]",
    )
    .await;
    site::ActiveModel {
        site_id: Set(site_id),
        top_bar_page: Set("include-top".into()),
        side_bar_page: Set("include-side".into()),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
    import_page(&runner, site_id, "include-nav-reader", "Body").await;
    let page = PageService::get(
        runner.context(),
        site_id,
        Reference::Slug("include-nav-reader".into()),
    )
    .await
    .unwrap();
    let revision =
        PageRevisionService::get_latest(runner.context(), site_id, page.page_id)
            .await
            .unwrap();
    let top = TextService::get(
        runner.context(),
        &revision.compiled_top_bar_html_hash.unwrap(),
    )
    .await
    .unwrap();
    let side = TextService::get(
        runner.context(),
        &revision.compiled_side_bar_html_hash.unwrap(),
    )
    .await
    .unwrap();
    assert!(top.contains("<strong>Top navigation</strong>"), "{top}");
    assert!(side.contains("<strong>Side navigation</strong>"), "{side}");
}

#[tokio::test]
async fn cyclic_includes_fail_without_replacing_the_stored_revision() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site":"test"}))
        .unwrap()
        .site
        .site_id;
    import_page(&runner, site_id, "include-cycle-a", "Initial A").await;
    import_page(
        &runner,
        site_id,
        "include-cycle-b",
        "[[include include-cycle-a]]",
    )
    .await;
    let page = PageService::get(
        runner.context(),
        site_id,
        Reference::Slug("include-cycle-a".into()),
    )
    .await
    .unwrap();
    let revision =
        PageRevisionService::get_latest(runner.context(), site_id, page.page_id)
            .await
            .unwrap();
    let source_hash =
        TextService::create(runner.context(), "[[include include-cycle-b]]".into())
            .await
            .unwrap();
    page_revision::ActiveModel {
        revision_id: Set(revision.revision_id),
        wikitext_hash: Set(source_hash.to_vec()),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
    let error = PageRevisionService::rerender(
        runner.context(),
        PageId::from_page_model(&page),
        RerenderType::Full,
    )
    .await
    .expect_err("cycle must terminate explicitly");
    assert!(format!("{error:?}").contains("nesting limit"), "{error:?}");
    let after = PageRevisionService::get_latest(runner.context(), site_id, page.page_id)
        .await
        .unwrap();
    assert_eq!(after.revision_id, revision.revision_id);
    assert_eq!(
        after.compiled_body_html_hash,
        revision.compiled_body_html_hash
    );
    assert_eq!(after.wikitext_hash, source_hash.to_vec());
}

#[tokio::test]
async fn a_chain_at_the_nesting_limit_renders_its_terminal_source() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site":"test"}))
        .unwrap()
        .site
        .site_id;
    for index in 0..=16 {
        import_page(
            &runner,
            site_id,
            &format!("include-depth-{index}"),
            "Depth endpoint",
        )
        .await;
    }
    for index in 0..16 {
        let page = PageService::get(
            runner.context(),
            site_id,
            Reference::Slug(format!("include-depth-{index}").into()),
        )
        .await
        .unwrap();
        let revision =
            PageRevisionService::get_latest(runner.context(), site_id, page.page_id)
                .await
                .unwrap();
        let hash = TextService::create(
            runner.context(),
            format!("[[include include-depth-{}]]", index + 1),
        )
        .await
        .unwrap();
        page_revision::ActiveModel {
            revision_id: Set(revision.revision_id),
            wikitext_hash: Set(hash.to_vec()),
            ..Default::default()
        }
        .update(runner.context().transaction())
        .await
        .unwrap();
    }
    let page = PageService::get(
        runner.context(),
        site_id,
        Reference::Slug("include-depth-0".into()),
    )
    .await
    .unwrap();
    PageRevisionService::rerender(
        runner.context(),
        PageId::from_page_model(&page),
        RerenderType::Full,
    )
    .await
    .expect("sixteen include levels must reach the terminal source");
    let revision =
        PageRevisionService::get_latest(runner.context(), site_id, page.page_id)
            .await
            .unwrap();
    let html = TextService::get(runner.context(), &revision.compiled_body_html_hash)
        .await
        .unwrap();
    assert!(html.contains("Depth endpoint"), "{html}");
}
