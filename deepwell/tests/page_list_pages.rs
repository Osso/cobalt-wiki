//! Live form templates and ListPages modules render into shared page output.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::models::{page_revision, role_permission};
use deepwell::services::page::CreatePage;
use deepwell::services::page_revision::RerenderType;
use deepwell::services::{PageRevisionService, PageService};
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
    let record = "author: Mishell\nrating: rated-m\ncontent: \"Dear //Slicket//\"\n";
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
        html.contains("Box by <strong>Mishell</strong> rated M for Mature"),
        "{html}"
    );
    assert!(html.contains("Dear <em>Slicket</em>"), "{html}");
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
