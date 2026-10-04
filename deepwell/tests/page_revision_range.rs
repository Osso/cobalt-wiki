//! Native revision history: ranges page from the newest revision backwards,
//! and hiding revision fields accepts only fields that readers understand.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::prelude::*;
use deepwell::services::RequestContext;
use deepwell::types::Reference;
use serde_json::json;

const SLUG: &str = "revision-range";

/// A page with revisions 0..=4 and the ID of each.
async fn page_with_five_revisions(runner: &mut TestRunner) -> (i64, i64, Vec<i64>) {
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    runner.set_request_context(RequestContext {
        session: None,
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug(SLUG.into())),
    });
    let created = run_endpoint!(
        runner,
        page_create,
        json!({
            "site_id": site_id, "wikitext": "Body", "title": "Title 0", "alt_title": null,
            "slug": SLUG, "layout": null, "revision_comments": "Create",
            "user_id": ADMIN_USER_ID, "ip_address": common::IP_ADDRESS,
        }),
    );
    let mut revisions = vec![created.revision_id];
    for number in 1..=4 {
        let edited = run_endpoint!(
            runner,
            page_edit,
            json!({
                "site_id": site_id, "page": created.page_id,
                "last_revision_id": revisions.last().unwrap(),
                "revision_comments": format!("Edit {number}"), "title": format!("Title {number}"),
                "user_id": ADMIN_USER_ID, "ip_address": common::IP_ADDRESS,
            }),
        )
        .expect("edit creates a revision");
        revisions.push(edited.revision_id);
    }
    (site_id, created.page_id, revisions)
}

async fn range(
    runner: &TestRunner,
    site_id: i64,
    page_id: i64,
    number: i32,
    direction: &str,
) -> Vec<i32> {
    run_endpoint!(
        runner,
        page_revision_range,
        json!({
            "site_id": site_id, "page_id": page_id, "revision_number": number,
            "revision_direction": direction, "limit": 2,
        }),
    )
    .into_iter()
    .map(|revision| revision.revision_number)
    .collect()
}

#[tokio::test]
async fn revision_ranges_take_the_nearest_revisions_in_each_direction() {
    let mut runner = TestRunner::setup().await;
    let (site_id, page_id, _) = page_with_five_revisions(&mut runner).await;

    assert_eq!(range(&runner, site_id, page_id, -1, "before").await, [3, 4]);
    assert_eq!(range(&runner, site_id, page_id, 2, "before").await, [1, 2]);
    assert_eq!(range(&runner, site_id, page_id, 1, "after").await, [1, 2]);
}

#[tokio::test]
async fn hiding_rejects_unknown_fields_and_other_pages_revisions() {
    let mut runner = TestRunner::setup().await;
    let (site_id, page_id, revisions) = page_with_five_revisions(&mut runner).await;
    let hide = |page_id: i64, revision_id: i64, hidden: &[&str]| {
        json!({
            "site_id": site_id, "page_id": page_id, "revision_id": revision_id,
            "user_id": ADMIN_USER_ID, "hidden": hidden,
        })
    };

    let error = run_endpoint_err!(
        runner,
        page_revision_edit,
        hide(page_id, revisions[1], &["comments", "bogus"]),
    );
    assert_contains_error!(error, ErrorType::BadRequest);
    let error = run_endpoint_err!(
        runner,
        page_revision_edit,
        hide(page_id + 1_000_000, revisions[1], &["comments"]),
    );
    assert_contains_error!(error, ErrorType::PageRevisionNotFound);

    // Nothing was hidden, and the revision is still readable.
    assert_eq!(range(&runner, site_id, page_id, 1, "after").await, [1, 2]);

    let hidden = run_endpoint!(
        runner,
        page_revision_edit,
        hide(page_id, revisions[1], &["comments"]),
    );
    assert_eq!(hidden.hidden, ["comments"]);
    assert_eq!(hidden.comments, None);
}
