//! A bare `[[gallery]]` shows the page's image attachments in Wikidot's order.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::page::CreatePage;
use deepwell::services::{PageService, RequestContext};
use deepwell::types::Reference;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::json;

#[tokio::test]
async fn bare_gallery_lists_page_images_in_wikidot_order() {
    let mut runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    runner.set_request_context(RequestContext {
        session: None,
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug("badges".into())),
    });
    let page = PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            slug: "badges".into(),
            title: "Badges".into(),
            wikitext: "No gallery yet".into(),
            alt_title: None,
            tags: vec![],
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Gallery fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    // badges on Wikidot lists badge_profession_none.png, then _@@.png, then
    // _skinning.png: punctuation and case do not sort. The text file and the
    // deleted image are not shown.
    runner
        .context()
        .transaction()
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "WITH files (name, mime, deleted) AS (VALUES
                 ('badge_profession_skinning.png', 'image/png', false),
                 ('badge_profession_@@.png', 'image/png', false),
                 ('Badge_Neutral-Ring.png', 'image/png', false),
                 ('badge_profession_none.png', 'image/png', false),
                 ('notes.txt', 'text/plain', false),
                 ('badge_deleted.png', 'image/png', true)
             ), inserted AS (
                 INSERT INTO file (name, page_id, site_id, deleted_at)
                 SELECT name, $1, $2, CASE WHEN deleted THEN now() END FROM files
                 RETURNING file_id, name
             )
             INSERT INTO file_revision
                 (revision_type, revision_number, file_id, page_id, site_id, user_id,
                  name, s3_hash, mime, size, changes, comments)
             SELECT 'create', 0, i.file_id, $1, $2, $3, i.name,
                    decode(repeat('00', 64), 'hex'), f.mime, 1,
                    '{page,name,blob,mime}', ''
             FROM inserted i JOIN files f USING (name)",
            [page.page_id.into(), site_id.into(), ADMIN_USER_ID.into()],
        ))
        .await
        .unwrap();
    run_endpoint!(
        runner,
        page_edit,
        json!({
            "site_id": site_id, "page": page.page_id,
            "last_revision_id": page.revision_id,
            "revision_comments": "Add gallery", "user_id": ADMIN_USER_ID,
            "wikitext": "[[gallery size=\"square\"]]", "ip_address": common::IP_ADDRESS
        })
    )
    .unwrap();

    let html = run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": page.page_id,
            "details": {"compiled": true}
        })
    )
    .unwrap()
    .compiled_body_html
    .unwrap();
    let images = html
        .split("<img src=\"")
        .skip(1)
        .map(|img| img.split('"').next().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        images,
        [
            "/-/file/badges/Badge_Neutral-Ring.png",
            "/-/file/badges/badge_profession_none.png",
            "/-/file/badges/badge_profession_@@.png",
            "/-/file/badges/badge_profession_skinning.png",
        ],
        "{html}"
    );
    assert!(
        html.contains("<div class=\"gallery-item square\">"),
        "{html}"
    );
}
