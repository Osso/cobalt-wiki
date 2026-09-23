//! SiteChanges lists Wikidot's revision list, newest first, 20 per page.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::PageService;
use deepwell::services::page::CreatePage;
use deepwell::services::view::GetPageViewOutput;
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

#[tokio::test]
async fn site_changes_list_revisions_newest_first_with_pages() {
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
}
