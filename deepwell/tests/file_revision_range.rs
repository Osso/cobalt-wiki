//! File revision history pages from the newest revision backwards, as page
//! history does. Revision rows are inserted directly: no S3 is needed to list.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::RequestContext;
use deepwell::services::file_revision::{FileRevisionService, GetFileRevisionRange};
use deepwell::types::{FetchDirection, Reference};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::json;

const SLUG: &str = "file-revision-range";

async fn file_with_five_revisions(runner: &mut TestRunner) -> i64 {
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
    let page_id = run_endpoint!(
        runner,
        page_create,
        json!({
            "site_id": site_id, "wikitext": "Files", "title": "Files", "alt_title": null,
            "slug": SLUG, "layout": null, "revision_comments": "Create",
            "user_id": ADMIN_USER_ID, "ip_address": common::IP_ADDRESS,
        }),
    )
    .page_id;
    let db = runner.context().transaction();
    let file_id: i64 = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO file (name, page_id, site_id) VALUES ('history.txt', $1, $2) RETURNING file_id",
            [page_id.into(), site_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "file_id")
        .unwrap();
    for number in 0..5 {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO file_revision (revision_type, revision_number, file_id, page_id, site_id, user_id, name, s3_hash, mime, size, comments, changes) \
             VALUES ($1, $2, $3, $4, $5, $6, 'history.txt', decode(repeat('ab', 64), 'hex'), 'text/plain', 5, '', \
             CASE WHEN $2 = 0 THEN '{page,name,blob,mime}'::text[] ELSE '{blob}'::text[] END)",
            [
                (if number == 0 { "create" } else { "regular" }).into(),
                number.into(),
                file_id.into(),
                page_id.into(),
                site_id.into(),
                ADMIN_USER_ID.into(),
            ],
        ))
        .await
        .unwrap();
    }
    file_id
}

async fn range(
    runner: &TestRunner,
    file_id: i64,
    revision_number: i32,
    revision_direction: FetchDirection,
) -> Vec<i32> {
    FileRevisionService::get_range(
        runner.context(),
        GetFileRevisionRange {
            file_id,
            revision_number,
            revision_direction,
            limit: 2,
        },
    )
    .await
    .unwrap()
    .into_iter()
    .map(|revision| revision.revision_number)
    .collect()
}

#[tokio::test]
async fn file_revision_ranges_take_the_nearest_revisions_in_each_direction() {
    let mut runner = TestRunner::setup().await;
    let file_id = file_with_five_revisions(&mut runner).await;

    assert_eq!(
        range(&runner, file_id, -1, FetchDirection::Before).await,
        [3, 4]
    );
    assert_eq!(
        range(&runner, file_id, 2, FetchDirection::Before).await,
        [1, 2]
    );
    assert_eq!(
        range(&runner, file_id, 1, FetchDirection::After).await,
        [1, 2]
    );
}
