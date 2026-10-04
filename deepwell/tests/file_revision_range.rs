//! File revision history pages from the newest revision backwards, as page
//! history does, and revision edits stay within their file. Revision rows
//! are inserted directly: no S3 is needed for either.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::prelude::*;
use deepwell::models::file_revision;
use deepwell::services::RequestContext;
use deepwell::services::file_revision::{
    FileRevisionService, GetFileRevisionRange, UpdateFileRevision,
};
use deepwell::types::{FetchDirection, Reference};
use sea_orm::{ConnectionTrait, DbBackend, EntityTrait, Statement};
use serde_json::json;

const SLUG: &str = "file-revision-range";

async fn page(runner: &mut TestRunner) -> (i64, i64) {
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
    (site_id, page_id)
}

/// A file with `count` revisions; returns its ID and the revision IDs in order.
async fn insert_file(
    runner: &TestRunner,
    (site_id, page_id): (i64, i64),
    name: &str,
    count: i32,
) -> (i64, Vec<i64>) {
    let db = runner.context().transaction();
    let file_id: i64 = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO file (name, page_id, site_id) VALUES ($1, $2, $3) RETURNING file_id",
            [name.into(), page_id.into(), site_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "file_id")
        .unwrap();
    let mut revisions = Vec::new();
    for number in 0..count {
        let revision_id: i64 = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO file_revision (revision_type, revision_number, file_id, page_id, site_id, user_id, name, s3_hash, mime, size, comments, changes) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, decode(repeat('ab', 64), 'hex'), 'text/plain', 5, '', \
                 CASE WHEN $2 = 0 THEN '{page,name,blob,mime}'::text[] ELSE '{blob}'::text[] END) \
                 RETURNING revision_id",
                [
                    (if number == 0 { "create" } else { "regular" }).into(),
                    number.into(),
                    file_id.into(),
                    page_id.into(),
                    site_id.into(),
                    ADMIN_USER_ID.into(),
                    name.into(),
                ],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "revision_id")
            .unwrap();
        revisions.push(revision_id);
    }
    (file_id, revisions)
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
    let page = page(&mut runner).await;
    let (file_id, _) = insert_file(&runner, page, "history.txt", 5).await;

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

#[tokio::test]
async fn file_revision_edit_only_changes_revisions_of_that_file() {
    let mut runner = TestRunner::setup().await;
    let page = page(&mut runner).await;
    let (file_a, _) = insert_file(&runner, page, "a.txt", 3).await;
    let (_, revisions_b) = insert_file(&runner, page, "b.txt", 3).await;
    let (site_id, page_id) = page;
    let hide = |file_id: i64, revision_id: i64| UpdateFileRevision {
        site_id,
        page_id,
        file_id,
        revision_id,
        user_id: ADMIN_USER_ID,
        hidden: vec!["comments".into()],
    };

    // File B's old revision, addressed through file A.
    let error =
        FileRevisionService::update(runner.context(), hide(file_a, revisions_b[0]))
            .await
            .unwrap_err();
    assert_eq!(error.error_type, ErrorType::FileRevisionNotFound);
    let untouched = file_revision::Entity::find_by_id(revisions_b[0])
        .one(runner.context().transaction())
        .await
        .unwrap()
        .unwrap();
    assert!(
        untouched.hidden.is_empty(),
        "file B's revision is unchanged"
    );
}
