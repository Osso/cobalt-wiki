//! Bulk rename through registered RPCs and native form/include/ListPages rendering.
#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::ErrorType;
use deepwell::services::{PageRevisionService, RequestContext};
use deepwell::types::Reference;
use sea_orm::ConnectionTrait;
use serde_json::json;

const TEMPLATE: &str = "[[include writingbox | characters=%%form_raw{characters}%%]]\n\nCopy tags: %%form_raw{characters}%%\n\n====\n\n[[form]]\nfields:\n  characters:\n    type: text\n  content:\n    type: wiki\n[[/form]]";
const SOURCE: &str = "characters: ' ,OLD, oldish\tkeep  old,,OLD, '\ncontent: 'old biography stays old'\nunknown: true\ncount: 7\nempty: null\n";
const AFTER: &str = " ,new, oldish\tkeep  new,,new, ";

async fn setup() -> (TestRunner, i64) {
    let mut runner = TestRunner::setup_with_idle_job_workers().await;
    // Isolated native fixture predates tag-admin tables. Exercise current schema
    // inside this rollback-only transaction, without changing the shared fixture.
    runner
        .context()
        .transaction()
        .execute_unprepared(include_str!(
            "../migrations/20260928000000_site_tag_operation.sql"
        ))
        .await
        .unwrap();
    runner
        .context()
        .transaction()
        .execute_unprepared(include_str!(
            "../migrations/20260929000000_site_tag_add.sql"
        ))
        .await
        .unwrap();
    let site = run_endpoint!(runner, site_get, json!({"site":"test"}))
        .unwrap()
        .site
        .site_id;
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        ..Default::default()
    });
    import(&runner, site, "writingbox", "[[module ListPages category=\"character\" tags=\"{$characters}\" separate=\"no\" order=\"name\"]]\n[[image icons/icon_%%name%%.jpg alt=\"%%title%%\"]]\n[[/module]]", &[]).await;
    import(&runner, site, "writing:_template", TEMPLATE, &[]).await;
    for name in ["new", "keep"] {
        import(
            &runner,
            site,
            &format!("character:{name}"),
            "Portrait fixture",
            &[name],
        )
        .await;
    }
    (runner, site)
}

async fn import(
    runner: &TestRunner,
    site: i64,
    slug: &str,
    source: &str,
    tags: &[&str],
) -> i64 {
    run_endpoint!(runner, page_import, json!({
        "site_id":site, "user_id":ADMIN_USER_ID, "slug":slug, "title":slug,
        "wikitext":source, "alt_title":null, "layout":"wikidot", "tags":tags,
        "bypass_filter":true, "revision_comments":"fixture", "ip_address":common::IP_ADDRESS
    })).page_id
}

async fn read(
    runner: &TestRunner,
    site: i64,
    page: i64,
) -> deepwell::services::page::GetPageOutput {
    run_endpoint!(
        runner,
        page_get,
        json!({"site_id":site, "page":page,
        "details":{"wikitext":true,"compiled":true}})
    )
    .unwrap()
}

#[tokio::test]
async fn rename_updates_tagged_and_drifted_character_fields_in_one_revision_and_undo() {
    let (runner, site) = setup().await;
    let tagged = import(&runner, site, "writing:tagged", SOURCE, &["old", "other"]).await;
    let drifted =
        import(&runner, site, "writing:drifted", SOURCE, &["new", "other"]).await;
    let unaffected_source = "characters: 'oldish keep'\ncontent: old\nunknown: false";
    let unaffected = import(
        &runner,
        site,
        "writing:unaffected",
        unaffected_source,
        &["other"],
    )
    .await;
    let biography = import(
        &runner,
        site,
        "character:biography",
        "characters: old\ncontent: old",
        &["other"],
    )
    .await;
    let before_tagged = read(&runner, site, tagged).await;
    let before_drifted = read(&runner, site, drifted).await;
    let before_unaffected = read(&runner, site, unaffected).await;
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":" OLD ","target_tag":" NEW "})
    );
    assert_eq!(
        preview.pages.iter().map(|p| p.page_id).collect::<Vec<_>>(),
        [tagged, drifted],
        "must select stale form-only references too"
    );
    let applied = run_endpoint!(
        runner,
        site_tag_apply,
        json!({"source_tag":"OLD", "target_tag":"NEW", "preview_token":preview.preview_token,"ip_address":common::IP_ADDRESS})
    );
    assert_eq!(applied.count, 2);
    for (id, before, expected_changes) in [
        (tagged, &before_tagged, vec!["tags", "wikitext"]),
        (drifted, &before_drifted, vec!["wikitext"]),
    ] {
        let after = read(&runner, site, id).await;
        assert_eq!(after.tags, ["new", "other"]);
        let values =
            wikidot_forms::parse_values(after.wikitext.as_deref().unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(values).unwrap(),
            json!({"characters":AFTER,"content":"old biography stays old","unknown":true,"count":7,"empty":null})
        );
        let revision = PageRevisionService::get_latest(runner.context(), site, id)
            .await
            .unwrap();
        let original =
            PageRevisionService::get_direct(runner.context(), before.revision_id)
                .await
                .unwrap();
        assert_eq!(revision.revision_number, original.revision_number + 1);
        assert_eq!(revision.changes, expected_changes);
        let html = after.compiled_body_html.unwrap();
        assert_eq!(html.matches("<img ").count(), 2, "{html}");
        assert!(html.contains("alt=\"character:new\""), "{html}");
        assert!(html.contains("alt=\"character:keep\""), "{html}");
        assert!(!html.contains("alt=\"character:old\""), "{html}");
        assert!(html.contains("Copy tags:  ,new, oldish"), "{html}");
    }
    assert_eq!(
        read(&runner, site, unaffected).await.revision_id,
        before_unaffected.revision_id
    );
    assert_eq!(
        read(&runner, site, unaffected).await.wikitext.as_deref(),
        Some(unaffected_source)
    );
    assert_eq!(
        read(&runner, site, biography).await.wikitext.as_deref(),
        Some("characters: old\ncontent: old")
    );
    run_endpoint!(
        runner,
        site_tag_undo,
        json!({"operation_id":applied.operation_id,"ip_address":common::IP_ADDRESS})
    );
    for (id, before) in [(tagged, before_tagged), (drifted, before_drifted)] {
        let restored = read(&runner, site, id).await;
        assert_eq!(restored.wikitext, before.wikitext);
        assert_eq!(restored.tags, before.tags);
    }
}

#[tokio::test]
async fn field_only_rename_works_without_any_source_tag_and_stale_preview_is_atomic() {
    let (mut runner, site) = setup().await;
    let page = import(&runner, site, "writing:only", SOURCE, &["new"]).await;
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"old", "target_tag":"new"})
    );
    assert_eq!(preview.count, 1);
    let before = read(&runner, site, page).await;
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(page)),
        ..Default::default()
    });
    run_endpoint!(
        runner,
        page_edit,
        json!({"site_id":site,"page":page,"user_id":ADMIN_USER_ID,"last_revision_id":before.revision_id,"form_updates":{"characters":"old keep"},"revision_comments":"concurrent field edit","ip_address":common::IP_ADDRESS})
    );
    let newer = read(&runner, site, page).await;
    let error = run_endpoint_err!(
        runner,
        site_tag_apply,
        json!({"source_tag":"old","target_tag":"new","preview_token":preview.preview_token,"ip_address":common::IP_ADDRESS})
    );
    assert_contains_error!(error, ErrorType::TagPreviewStale);
    assert_eq!(
        read(&runner, site, page).await.revision_id,
        newer.revision_id
    );
    assert!(run_endpoint!(runner, site_tag_history).is_empty());
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"old","target_tag":"new"})
    );
    run_endpoint!(
        runner,
        site_tag_apply,
        json!({"source_tag":"old","target_tag":"new","preview_token":preview.preview_token,"ip_address":common::IP_ADDRESS})
    );
    let after = read(&runner, site, page).await;
    let values = wikidot_forms::parse_values(after.wikitext.as_deref().unwrap()).unwrap();
    assert_eq!(values.get("characters").unwrap().as_str(), Some("new keep"));
    assert_eq!(after.tags, ["new"]);
}

#[tokio::test]
async fn delete_and_nonstring_characters_preserve_source() {
    let (runner, site) = setup().await;
    let text = import(&runner, site, "writing:delete", SOURCE, &["old"]).await;
    let numeric_source = "characters: 7\ncontent: old\nunknown: true";
    let numeric = import(&runner, site, "writing:number", numeric_source, &["old"]).await;
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"old","target_tag":null})
    );
    run_endpoint!(
        runner,
        site_tag_apply,
        json!({"source_tag":"old","target_tag":null,"preview_token":preview.preview_token,"ip_address":common::IP_ADDRESS})
    );
    assert_eq!(
        read(&runner, site, text).await.wikitext.as_deref(),
        Some(SOURCE)
    );
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"7","target_tag":"new"})
    );
    assert_eq!(preview.count, 0);
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"old","target_tag":"new"})
    );
    assert_eq!(
        preview.count, 1,
        "deletion leaves characters refs eligible for later rename"
    );
    assert_eq!(
        read(&runner, site, numeric).await.wikitext.as_deref(),
        Some(numeric_source)
    );
}
