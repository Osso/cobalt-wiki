//! SiteChanges lists Wikidot's revision list, newest first, 20 per page.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::PageService;
use deepwell::services::page::CreatePage;
use deepwell::services::view::GetPageViewOutput;
use scraper::{ElementRef, Html, Selector};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::json;

async fn view(runner: &TestRunner, site_id: i64, extra: &str) -> String {
    let output = deepwell::endpoints::view::page_view(
        runner.context(),
        common::make_params(json!({
            "site_id": site_id, "session_token": null, "locales": ["en"],
            "route": {"slug": "changes:recent", "extra": extra},
        })),
    )
    .await
    .expect("view the changes page");
    match output {
        GetPageViewOutput::Found {
            compiled_body_html, ..
        } => compiled_body_html,
        other => panic!("page must be found: {other:?}"),
    }
}

async fn setup_changes() -> (TestRunner, i64) {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    // 25 revisions of one page, a minute apart; revision 0 is its creation.
    runner
        .context()
        .transaction()
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO wikidot_site_change
               (site_id, page_slug, page_title, revision_number, flags, changed_at,
                user_slug, user_name, source_user_id, comments)
             SELECT $1, 'writing:changes-fixture', 'Changes Fixture', n,
                    CASE WHEN n = 0 THEN 'N' ELSE 'S' END,
                    to_timestamp(1790000000 + n * 60), 'allicat', 'Allicat', 7570574,
                    CASE WHEN n = 24 THEN 'Latest edit.' ELSE '' END
             FROM generate_series(0, 24) AS n",
            [site_id.into()],
        ))
        .await
        .unwrap();
    PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            slug: "changes:recent".into(),
            title: "Recent changes".into(),
            wikitext: "[[module SiteChanges]]".into(),
            alt_title: None,
            tags: vec![],
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "SiteChanges fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    (runner, site_id)
}

#[tokio::test]
async fn site_changes_list_revisions_newest_first_with_pages() {
    let (runner, site_id) = setup_changes().await;
    let first = view(&runner, site_id, "").await;
    let revisions = |html: &str| {
        html.split("<td class=\"revision-no\">")
            .skip(1)
            .map(|cell| cell.split("</td>").next().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let listed = revisions(&first);
    assert_eq!(listed.len(), 20, "{first}");
    assert_eq!(listed[0], "(rev. 24)");
    assert_eq!(listed[19], "(rev. 5)");
    assert!(first.contains("<div class=\"comments\">Latest edit.</div>"));
    assert!(first.contains("<span class=\"pager-no\">page 1</span>"));
    assert!(first.contains("<a href=\"/changes:recent/p/2\">next &raquo;</a>"));

    let second = view(&runner, site_id, "p/2").await;
    assert_eq!(
        revisions(&second),
        ["(rev. 4)", "(rev. 3)", "(rev. 2)", "(rev. 1)", "(new)"]
    );
    assert!(second.contains("title=\"new page created\">N</span>"));

    // Wikidot's filters, from the URL: revision type, revisions per page, category.
    let new_pages = view(&runner, site_id, "p/1/types/N").await;
    assert_eq!(revisions(&new_pages), ["(new)"]);
    assert!(
        new_pages.contains("id=\"rev-type-new\" data-flag=\"N\" checked=\"checked\"")
    );

    let ten = view(&runner, site_id, "p/1/perpage/10").await;
    assert_eq!(revisions(&ten).len(), 10);
    assert!(
        ten.contains("<a href=\"/changes:recent/p/2/perpage/10\">next &raquo;</a>"),
        "{ten}"
    );
    assert!(ten.contains("<option value=\"10\" selected=\"selected\">10</option>"));

    assert_eq!(
        revisions(&view(&runner, site_id, "p/1/category/writing").await).len(),
        20
    );
    assert!(
        revisions(&view(&runner, site_id, "p/1/category/character").await).is_empty()
    );
}

fn select_text(element: &ElementRef<'_>, selector: &str) -> Vec<String> {
    element
        .select(&Selector::parse(selector).unwrap())
        .map(|node| node.text().collect::<String>().trim().to_owned())
        .collect()
}

fn change_rows(html: &Html) -> Vec<ElementRef<'_>> {
    html.select(
        &Selector::parse(".site-changes-table tbody tr.changes-list-item").unwrap(),
    )
    .collect()
}

#[tokio::test]
async fn site_changes_render_valid_table_with_explicit_utc_dates_and_escaped_content() {
    let (runner, site_id) = setup_changes().await;
    runner
        .context()
        .transaction()
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE wikidot_site_change
             SET page_title = 'A <title> & \"quote\"',
                 user_slug = NULL, user_name = 'Guest <writer> & Friend',
                 comments = 'Changed <script> & \"notes\"'
             WHERE site_id = $1 AND revision_number = 24",
            [site_id.into()],
        ))
        .await
        .unwrap();

    let output = view(&runner, site_id, "").await;
    let html = Html::parse_fragment(&output);
    let table = html
        .select(&Selector::parse("table.site-changes-table").unwrap())
        .next()
        .expect("SiteChanges must render a table");
    assert_eq!(
        select_text(&table, "thead th"),
        ["Page", "Changes", "Revision", "Changed", "Author"]
    );
    let rows = change_rows(&html);
    assert_eq!(rows.len(), 20);
    for row in &rows {
        assert_eq!(select_text(row, "td").len(), 5, "{row:?}");
        assert_eq!(select_text(row, "td.title a").len(), 1);
        assert_eq!(select_text(row, "td.flags").len(), 1);
        assert_eq!(select_text(row, "td.revision-no").len(), 1);
        assert_eq!(
            select_text(row, "td.mod-date time.site-change-date").len(),
            1
        );
        assert_eq!(select_text(row, "td.mod-by").len(), 1);
    }
    let first = &rows[0];
    assert_eq!(
        select_text(first, "td.title a"),
        ["writing: A <title> & \"quote\""]
    );
    assert_eq!(
        select_text(first, "td.title .comments"),
        ["Changed <script> & \"notes\""]
    );
    assert_eq!(select_text(first, "td.mod-by"), ["Guest <writer> & Friend"]);
    assert_eq!(
        first
            .select(&Selector::parse("td.mod-by a").unwrap())
            .count(),
        0
    );
    assert_eq!(select_text(first, "td.revision-no"), ["(rev. 24)"]);
    assert_eq!(first.select(&Selector::parse("script").unwrap()).count(), 0);
    let date = first
        .select(&Selector::parse("td.mod-date time.site-change-date").unwrap())
        .next()
        .unwrap();
    assert_eq!(date.value().attr("datetime"), Some("2026-09-21T14:37:20Z"));
    assert_eq!(date.value().attr("data-timestamp"), Some("1790001440"));
    assert!(date.text().collect::<String>().contains("14:37:20"));
    assert!(date.text().collect::<String>().contains("UTC"));
    assert!(
        first
            .select(&Selector::parse(".odate").unwrap())
            .next()
            .is_none()
    );
    assert_eq!(select_text(&rows[19], "td.revision-no"), ["(rev. 5)"]);
    assert_eq!(select_text(&html.root_element(), ".rowcount").len(), 1);

    let second = Html::parse_fragment(&view(&runner, site_id, "p/2").await);
    let second_rows = change_rows(&second);
    assert_eq!(second_rows.len(), 5);
    assert_eq!(select_text(&second_rows[4], "td.revision-no"), ["(new)"]);
    assert_eq!(
        second_rows[4]
            .select(&Selector::parse("td.mod-date time.site-change-date").unwrap())
            .next()
            .unwrap()
            .value()
            .attr("datetime"),
        Some("2026-09-21T14:13:20Z")
    );
}

#[tokio::test]
async fn site_changes_filter_flags_categories_and_page_sizes_with_preserved_pager_urls() {
    let (runner, site_id) = setup_changes().await;
    runner
        .context()
        .transaction()
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO wikidot_site_change
               (site_id, page_slug, page_title, revision_number, flags, changed_at,
                user_slug, user_name, source_user_id, comments)
             SELECT $1, slug, title, revision, flags, to_timestamp(changed),
                    'guest', 'Guest Writer', NULL, ''
             FROM (VALUES
                 ('writing:title', 'Title edit', 1, 'T', 1789999001),
                 ('writing:move', 'Move edit', 2, 'R', 1789999002),
                 ('writing:tags', 'Tags edit', 3, 'A', 1789999003),
                 ('writing:metadata', 'Metadata edit', 4, 'M', 1789999004),
                 ('writing:file', 'File edit', 5, 'F', 1789999005),
                 ('writing:both', 'Title and tags', 6, 'TA', 1789999006),
                 ('homepage', 'Default category', 7, 'S', 1789999007)
             ) AS changes(slug, title, revision, flags, changed)",
            [site_id.into()],
        ))
        .await
        .unwrap();

    for (flag, count) in [
        ('N', 1),
        ('S', 25),
        ('T', 2),
        ('R', 1),
        ('A', 2),
        ('M', 1),
        ('F', 1),
    ] {
        let page = view(&runner, site_id, &format!("p/1/perpage/50/types/{flag}")).await;
        let html = Html::parse_fragment(&page);
        assert_eq!(change_rows(&html).len(), count, "flag {flag}: {page}");
    }
    let combined = view(&runner, site_id, "p/1/types/TR").await;
    let combined_html = Html::parse_fragment(&combined);
    let combined_rows = change_rows(&combined_html);
    assert_eq!(combined_rows.len(), 3, "{combined}");
    assert_eq!(
        select_text(&combined_rows[0], "td.title a"),
        ["writing: Title and tags"]
    );
    assert_eq!(select_text(&combined_rows[0], "td.flags"), ["TA"]);

    for (category, count) in [("writing", 20), ("_default", 1), ("character", 0)] {
        let page = view(&runner, site_id, &format!("p/1/category/{category}")).await;
        let html = Html::parse_fragment(&page);
        assert_eq!(
            change_rows(&html).len(),
            count,
            "category {category}: {page}"
        );
        if category == "character" {
            assert!(
                html.root_element()
                    .text()
                    .collect::<String>()
                    .contains("Sorry, no revisions matching your criteria.")
            );
        }
    }
    let default_page =
        Html::parse_fragment(&view(&runner, site_id, "p/1/category/_default").await);
    assert_eq!(
        select_text(&change_rows(&default_page)[0], "td.title a"),
        ["Default category"]
    );

    for (size, count) in [(10, 10), (20, 20), (50, 32), (100, 32), (200, 32)] {
        let page = view(&runner, site_id, &format!("p/1/perpage/{size}")).await;
        let html = Html::parse_fragment(&page);
        assert_eq!(change_rows(&html).len(), count, "size {size}: {page}");
        let selected = html
            .select(&Selector::parse("#rev-perpage option:checked").unwrap())
            .next()
            .unwrap();
        assert_eq!(
            selected.value().attr("value"),
            Some(size.to_string().as_str())
        );
    }
    let filtered = "p/1/perpage/10/category/writing/types/S";
    let first = view(&runner, site_id, filtered).await;
    let first_html = Html::parse_fragment(&first);
    assert_eq!(change_rows(&first_html).len(), 10);
    assert!(
        first
            .contains("href=\"/changes:recent/p/2/perpage/10/category/writing/types/S\"")
    );
    let second = view(&runner, site_id, "p/2/perpage/10/category/writing/types/S").await;
    let second_html = Html::parse_fragment(&second);
    assert_eq!(change_rows(&second_html).len(), 10);
    assert!(
        second
            .contains("href=\"/changes:recent/p/1/perpage/10/category/writing/types/S\"")
    );
    assert!(
        second
            .contains("href=\"/changes:recent/p/3/perpage/10/category/writing/types/S\"")
    );
    let third = Html::parse_fragment(
        &view(&runner, site_id, "p/3/perpage/10/category/writing/types/S").await,
    );
    assert_eq!(change_rows(&third).len(), 4);
}
