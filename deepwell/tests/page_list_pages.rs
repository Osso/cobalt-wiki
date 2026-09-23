//! Live form templates and ListPages modules render into shared page output.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::{page_revision, role_permission};
use deepwell::services::page::CreatePage;
use deepwell::services::page_revision::RerenderType;
use deepwell::services::render::BodyArguments;
use deepwell::services::{PageRevisionService, PageService, TextService};
use deepwell::types::{Action, PageId, Reference, RerenderDepth, Resource};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;

const CHARACTER_TEMPLATE: &str = "Character sheet\n\n====\n\n[[form]]\nfields:\n  race:\n    type: select\n    values:\n      nightelf: Night Elf\n      human: Human\n[[/form]]\n";

async fn site_id(runner: &TestRunner) -> i64 {
    run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id
}

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
            revision_comments: "ListPages rendering fixture".into(),
            tags: vec![],
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .expect("import fixture page");
}

/// Give the latest revision its archived title and tags, as a later metadata edit would.
async fn set_title_and_tags(
    runner: &TestRunner,
    site_id: i64,
    slug: &str,
    title: &str,
    tags: &[&str],
) {
    let page = PageService::get(runner.context(), site_id, Reference::Slug(slug.into()))
        .await
        .unwrap();
    let revision =
        PageRevisionService::get_latest(runner.context(), site_id, page.page_id)
            .await
            .unwrap();
    page_revision::ActiveModel {
        revision_id: Set(revision.revision_id),
        title: Set(title.into()),
        tags: Set(tags.iter().map(|tag| tag.to_string()).collect()),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
}

async fn compiled_body(runner: &TestRunner, site_id: i64, slug: &str) -> String {
    let page = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": slug, "details": {"wikitext": true, "compiled": true}})
    )
    .unwrap();
    page.compiled_body_html.unwrap()
}

async fn rerender(runner: &TestRunner, site_id: i64, slug: &str) {
    let page = PageService::get(runner.context(), site_id, Reference::Slug(slug.into()))
        .await
        .unwrap();
    PageRevisionService::rerender(
        runner.context(),
        PageId::from_page_model(&page),
        RerenderDepth::default(),
        RerenderType::Full,
    )
    .await
    .unwrap();
}

async fn import_characters(runner: &TestRunner, site_id: i64) {
    import_page(runner, site_id, "character:_template", CHARACTER_TEMPLATE).await;
    let characters = [
        (
            "character:alpha",
            "Alpha [1]",
            "nightelf",
            &["_completed"][..],
        ),
        ("character:beta", "Beta", "human", &["_completed"][..]),
        (
            "character:gamma",
            "Gamma",
            "human",
            &["_completed", "inactive"][..],
        ),
        ("character:_hidden", "Hidden", "human", &["_completed"][..]),
        ("character:untagged", "Untagged", "human", &[][..]),
    ];
    for (slug, title, race, tags) in characters {
        import_page(runner, site_id, slug, &format!("race: {race}\n")).await;
        set_title_and_tags(runner, site_id, slug, title, tags).await;
    }
    import_page(runner, site_id, "writing:other", "Other category").await;
    set_title_and_tags(runner, site_id, "writing:other", "Other", &["_completed"]).await;
}

#[tokio::test]
async fn form_pages_render_through_their_category_template() {
    let runner = TestRunner::setup().await;
    let site_id = site_id(&runner).await;
    import_page(
        &runner,
        site_id,
        "writing-box",
        "Box by **{$author}** rated {$rating}",
    )
    .await;
    import_page(
        &runner,
        site_id,
        "writing:_template",
        "[[include writing-box | author=%%form_data{author}%% | rating=%%form_data{rating}%%]]\n\n%%form_raw{content}%%\n\n====\n\n[[form]]\nfields:\n  author:\n    type: text\n  rating:\n    type: select\n    values:\n      rated-t: T for Teen\n      rated-m: M for Mature\n  content:\n    type: wiki\n[[/form]]\n",
    )
    .await;
    let record = "author: 'Mishell -- //ooc//'\nrating: rated-m\ncontent: \"Dear //Slicket// -- Natlee\"\n";
    import_page(&runner, site_id, "writing:letter", record).await;
    import_page(&runner, site_id, "writing:_public", "Members only.").await;

    let page = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": "writing:letter", "details": {"wikitext": true, "compiled": true}})
    )
    .unwrap();
    assert_eq!(page.wikitext.as_deref(), Some(record));
    let html = page.compiled_body_html.unwrap();
    assert!(
        html.contains("Box by <strong><span style=\"white-space: pre-wrap;\">Mishell -- //ooc//</span></strong> rated <span style=\"white-space: pre-wrap;\">M for Mature</span>"),
        "{html}"
    );
    // Wiki fields are wikitext: `--` becomes a dash.
    assert!(
        html.contains("Dear <em>Slicket</em> \u{2014} Natlee"),
        "{html}"
    );
    for leaked in ["rating:", "rated-m", "[[form]]", "===="] {
        assert!(!html.contains(leaked), "{leaked} leaked: {html}");
    }

    let public = compiled_body(&runner, site_id, "writing:_public").await;
    assert!(public.contains("Members only."), "{public}");
    assert!(!public.contains("Box by"), "{public}");
}

#[tokio::test]
async fn list_pages_selects_orders_limits_and_fills_listed_page_tokens() {
    let runner = TestRunner::setup().await;
    let site_id = site_id(&runner).await;
    import_characters(&runner, site_id).await;
    let listing = "[[module ListPages category=\"character\" tags=\"_completed -inactive\" order=\"title desc\" separate=\"no\" prependLine=\"||~ Name ||~ Race ||\"]]\n|| %%linked_title%% || %%form_data{race}%% ||\n[[/module]]\n\n[[module ListPages category=\"character\" tags=\"_completed\" order=\"title\" limit=\"1\"]]\nFirst: %%title%% at %%link%%\n[[/module]]";
    import_page(&runner, site_id, "roster", listing).await;
    let html = compiled_body(&runner, site_id, "roster").await;

    let beta = html.find("Beta").expect(&html);
    let alpha = html.find("Alpha [1]").expect(&html);
    assert!(beta < alpha, "title desc must list Beta first: {html}");
    assert!(
        html.contains("<th>Name</th>") || html.contains(">Name<"),
        "{html}"
    );
    assert!(
        html.contains("Night Elf") && html.contains("Human"),
        "{html}"
    );
    assert!(html.contains("href=\"/character:alpha\""), "{html}");
    for excluded in ["Gamma", "Hidden", "Untagged", "Other"] {
        assert!(!html.contains(excluded), "{excluded} listed: {html}");
    }
    assert!(html.contains("class=\"list-pages-item\""), "{html}");
    assert_eq!(html.matches("First: ").count(), 1, "{html}");
    assert!(
        html.contains("First: Alpha [1] at /character:alpha"),
        "{html}"
    );
    assert!(!html.contains("%%") && !html.contains("[[module"), "{html}");
}

#[tokio::test]
async fn count_pages_fills_the_total_of_every_matching_page() {
    let runner = TestRunner::setup().await;
    let site_id = site_id(&runner).await;
    import_characters(&runner, site_id).await;
    for number in 0..21 {
        let slug = format!("arc:number-{number}");
        import_page(&runner, site_id, &slug, "Arc").await;
        set_title_and_tags(&runner, site_id, &slug, "Arc", &["_completed"]).await;
    }
    let stats = "[[module CountPages category=\"character\" tags=\"_completed\"]]\n%%total%% Character Profiles\n[[/module]]\n\n[[module CountPages category=\"character\" tags=\"_completed -inactive\"]]\n* %%total%% Active\n[[/module]]\n\n[[module CountPages category=\"arc\" tags=\"\"]]\n%%total%% Arcs\n[[/module]]";
    import_page(&runner, site_id, "stats", stats).await;
    let html = compiled_body(&runner, site_id, "stats").await;
    assert!(html.contains("<p>3 Character Profiles</p>"), "{html}");
    assert!(html.contains("<li>2 Active</li>"), "{html}");
    assert!(html.contains("<p>21 Arcs</p>"), "{html}");
    assert_eq!(
        html.matches("class=\"list-pages-box\"").count(),
        3,
        "{html}"
    );
}

#[tokio::test]
async fn nested_list_pages_list_each_outer_page_with_its_own_inner_pages() {
    let runner = TestRunner::setup().await;
    let site_id = site_id(&runner).await;
    import_characters(&runner, site_id).await;
    for (slug, title, tags) in [
        ("player:ann", "Ann", &["_completed"][..]),
        ("player:bob", "Bob", &["_completed"][..]),
    ] {
        import_page(&runner, site_id, slug, "Player").await;
        set_title_and_tags(&runner, site_id, slug, title, tags).await;
    }
    set_title_and_tags(&runner, site_id, "character:alpha", "Alpha", &["ann"]).await;
    set_title_and_tags(&runner, site_id, "character:beta", "Beta", &["bob"]).await;
    set_title_and_tags(&runner, site_id, "character:gamma", "Gamma", &["ann"]).await;
    // As on Cobalt's testlist, the inner module arrives through an include.
    import_page(
        &runner,
        site_id,
        "characterlist",
        "[[module ListPages category=\"character\" tags=\"{$tags}\" order=\"title\"]]\n* char %%title%%\n[[/module]]",
    )
    .await;
    let players = "[[module ListPages category=\"player\" tags=\"_completed\" order=\"title\" prependLine=\"Players:\"]]\n+ %%title%%\n[[include characterlist | tags=+%%name%%]]\n[[/module]]\nEnd";
    import_page(&runner, site_id, "testlist", players).await;
    let html = compiled_body(&runner, site_id, "testlist").await;

    let position =
        |text: &str| html.find(text).unwrap_or_else(|| panic!("{text}: {html}"));
    assert!(position("Ann") < position("char Alpha"));
    assert!(position("char Alpha") < position("char Gamma"));
    assert!(position("char Gamma") < position("Bob"));
    assert!(position("Bob") < position("char Beta"));
    assert!(position("char Beta") < position("End"));
    assert_eq!(html.matches("char ").count(), 3, "{html}");
    assert!(!html.contains("%%") && !html.contains("[[module"), "{html}");
}

#[tokio::test]
async fn list_pages_paginate_like_wikidot() {
    let runner = TestRunner::setup().await;
    let site_id = site_id(&runner).await;
    import_characters(&runner, site_id).await;
    // Three listable characters (Alpha, Beta, Gamma), two per page.
    let listing = "[[module ListPages category=\"character\" tags=\"_completed\" order=\"title\" perPage=\"2\"]]\n* %%title%%\n[[/module]]\n\n[[module ListPages category=\"character\" tags=\"_completed\" order=\"title\" perPage=\"2\" limit=\"2\"]]\n* capped %%title%%\n[[/module]]";
    import_page(&runner, site_id, "roster", listing).await;

    let first = compiled_body(&runner, site_id, "roster").await;
    assert!(first.contains("Alpha") && first.contains("Beta"), "{first}");
    assert!(!first.contains("Gamma"), "{first}");
    assert!(first.contains("page 1 of 2"), "{first}");
    assert!(first.contains("href=\"/roster/p/2\">next »</a>"), "{first}");
    // limit caps items across pages, so the second module has one page and no pager.
    assert_eq!(first.matches("class=\"pager\"").count(), 1, "{first}");

    let page =
        PageService::get(runner.context(), site_id, Reference::Slug("roster".into()))
            .await
            .unwrap();
    let second = PageRevisionService::render_body_view(
        runner.context(),
        site_id,
        page.page_id,
        &BodyArguments {
            list_page: 2,
            tag: None,
        },
    )
    .await
    .unwrap();
    assert!(second.contains("Gamma"), "{second}");
    assert!(
        !second.contains("Alpha [1]") && !second.contains(">Beta<"),
        "{second}"
    );
    assert!(second.contains("page 2 of 2"), "{second}");
    assert!(
        second.contains("href=\"/roster/p/1\">« previous</a>"),
        "{second}"
    );
    assert!(!second.contains("capped"), "{second}");
    // The stored page is still page 1.
    assert_eq!(compiled_body(&runner, site_id, "roster").await, first);
}

#[tokio::test]
async fn list_pages_never_lists_pages_denied_to_anonymous_readers() {
    let runner = TestRunner::setup().await;
    let site_id = site_id(&runner).await;
    import_characters(&runner, site_id).await;
    let listing = "[[module ListPages category=\"character\" order=\"title\"]]\n* %%title%%\n[[/module]]\nEnd of list";
    import_page(&runner, site_id, "roster", listing).await;
    assert!(
        compiled_body(&runner, site_id, "roster")
            .await
            .contains("Beta")
    );

    role_permission::Entity::delete_many()
        .filter(role_permission::Column::SiteId.eq(site_id))
        .filter(role_permission::Column::ResourceType.eq(Resource::Page))
        .filter(role_permission::Column::Action.eq(Action::View))
        .exec(runner.context().transaction())
        .await
        .unwrap();
    rerender(&runner, site_id, "roster").await;
    let html = compiled_body(&runner, site_id, "roster").await;
    assert!(html.contains("End of list"), "{html}");
    for title in ["Alpha", "Beta", "Gamma"] {
        assert!(!html.contains(title), "{title} listed: {html}");
    }
}

#[tokio::test]
async fn unsupported_list_pages_arguments_render_an_explicit_error() {
    let runner = TestRunner::setup().await;
    let site_id = site_id(&runner).await;
    import_page(
        &runner,
        site_id,
        "feed",
        "[[module ListPages rssTitle=\"Feed\"]]\n%%title%%\n[[/module]]",
    )
    .await;
    let html = compiled_body(&runner, site_id, "feed").await;
    assert!(
        html.contains("ListPages module error: unsupported argument rsstitle."),
        "{html}"
    );
}

/// Seeded test-site pages sharing an archived name take the archived source.
async fn import_or_replace_source(
    runner: &TestRunner,
    site_id: i64,
    slug: &str,
    source: String,
) {
    let existing = PageService::get_optional(
        runner.context(),
        site_id,
        Reference::Slug(slug.into()),
    )
    .await
    .unwrap();
    let Some(page) = existing else {
        return import_page(runner, site_id, slug, &source).await;
    };
    let revision =
        PageRevisionService::get_latest(runner.context(), site_id, page.page_id)
            .await
            .unwrap();
    let hash = TextService::create(runner.context(), source).await.unwrap();
    page_revision::ActiveModel {
        revision_id: Set(revision.revision_id),
        wikitext_hash: Set(hash.to_vec()),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
}

/// Render archived Cobalt pages against the whole archive imported in Wikidot page-ID order.
///
/// `COBALT_ARCHIVE_SOURCE` holds `<archive_key>.txt` sources, `COBALT_METADATA` the
/// metadata checkpoint, and `COBALT_RENDER_OUT` receives compiled HTML for inspection.
#[tokio::test]
#[ignore = "requires the protected Cobalt source archive"]
async fn archived_cobalt_pages_render_without_template_syntax() {
    let archive = std::env::var("COBALT_ARCHIVE_SOURCE").expect("archive directory");
    let output = std::env::var("COBALT_RENDER_OUT").expect("output directory");
    let metadata: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(std::env::var("COBALT_METADATA").expect("metadata"))
            .unwrap(),
    )
    .unwrap();
    let mut records: Vec<_> = metadata["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|record| record["status"] == "accepted")
        .collect();
    records.sort_by_key(|record| record["page_id"].as_i64().unwrap());

    let runner = TestRunner::setup().await;
    let site_id = site_id(&runner).await;
    for record in &records {
        let slug = record["fullname"].as_str().unwrap();
        let key = record["archive_key"].as_str().unwrap();
        let source = std::fs::read_to_string(format!("{archive}/{key}.txt")).unwrap();
        import_or_replace_source(&runner, site_id, slug, source).await;
        let tags: Vec<_> = record["tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tag| tag.as_str().unwrap())
            .collect();
        set_title_and_tags(
            &runner,
            site_id,
            slug,
            record["title"].as_str().unwrap(),
            &tags,
        )
        .await;
    }
    deepwell::models::site::ActiveModel {
        site_id: Set(site_id),
        top_bar_page: Set("nav:top".into()),
        side_bar_page: Set("nav:side".into()),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();

    let pages = [
        "start",
        "home:start",
        "character:atley",
        "writing:2005-04-02-a-brief-emphatic-response",
        "roster",
        "membersonly:players",
        "writings",
    ];
    for slug in pages {
        rerender(&runner, site_id, slug).await;
        let page =
            PageService::get(runner.context(), site_id, Reference::Slug(slug.into()))
                .await
                .unwrap();
        let revision =
            PageRevisionService::get_latest(runner.context(), site_id, page.page_id)
                .await
                .unwrap();
        let body = TextService::get(runner.context(), &revision.compiled_body_html_hash)
            .await
            .unwrap();
        let top = TextService::get(
            runner.context(),
            &revision.compiled_top_bar_html_hash.unwrap(),
        )
        .await
        .unwrap();
        let file = slug.replace(':', "_");
        std::fs::write(format!("{output}/{file}.html"), &body).unwrap();
        std::fs::write(format!("{output}/{file}.top.html"), &top).unwrap();
        for html in [&body, &top] {
            for leaked in [
                "%%",
                "[[module ListPages",
                "[[/module]]",
                "TODO: module ListPages",
            ] {
                assert!(!html.contains(leaked), "{slug}: {leaked} leaked");
            }
        }
    }
}
