//! Bulk rename through registered RPCs and native form/include/ListPages rendering.
#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::ErrorType;
use deepwell::services::{PageRevisionService, RequestContext};
use deepwell::types::Reference;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::json;

const TEMPLATE: &str = "[[include writingbox | characters=%%form_raw{characters}%%]]\n\nCopy tags: %%form_raw{characters}%%\n\n====\n\n[[form]]\nfields:\n  characters:\n    type: text\n  references:\n    type: text\n  content:\n    type: wiki\n[[/form]]";
// Production arc:_template and chain:_template feed `characters` to their portrait box.
const GROUP_TEMPLATE: &str = "[[include writingbox | characters=%%form_raw{characters}%%]]\n\n====\n\n[[form]]\nfields:\n  characters:\n    type: text\n[[/form]]";
const SOURCE: &str = "characters: ' ,OLD, oldish\tkeep  old,,OLD, '\ncontent: 'old biography stays old'\nunknown: true\ncount: 7\nempty: null\n";
const AFTER: &str = " ,new, oldish\tkeep  new,,new, ";

async fn setup() -> (TestRunner, i64) {
    let mut runner = TestRunner::setup_with_idle_job_workers().await;
    // Isolated native fixture predates tag-admin tables. Exercise current schema
    // inside this rollback-only transaction, without changing the shared fixture.
    let installed = runner
        .context()
        .transaction()
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT to_regclass('site_tag_operation') IS NOT NULL AS installed",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<bool>("", "installed")
        .unwrap();
    if !installed {
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
    }
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
async fn long_writing_characters_survive_form_edit_and_render_200_portraits() {
    let (mut runner, site) = setup().await;
    let names: Vec<String> = (0..200)
        .map(|index| format!("portrait{index:03}"))
        .collect();
    let characters: String = names
        .iter()
        .enumerate()
        .map(|(index, name)| match index {
            0 => name.clone(),
            index if index % 2 == 0 => format!(", {name}"),
            _ => format!(" {name}"),
        })
        .collect();
    assert!(characters.len() > 1_500);

    // Import fixtures in the rollback-only transaction, not through user-facing
    // create/edit flows (which would render and notify for every portrait).
    for (index, name) in names.iter().enumerate() {
        let category = if index % 2 == 0 { "character" } else { "bgc" };
        import(
            &runner,
            site,
            &format!("{category}:{name}"),
            "Portrait fixture",
            &[name.as_str()],
        )
        .await;
    }
    import(&runner, site, "long-writingbox", "[[module ListPages separate=\"no\" category=\"character bgc\" tags=\"{$characters}\" order=\"name\" perPage=\"200\" \\]]\n[[image icons/icon_%%name%%.jpg alt=\"%%title%%\"]]\n[[/module]]", &[]).await;
    import(&runner, site, "long-writing:_template", "[[include long-writingbox | characters=%%form_raw{characters}%%]]\n\n====\n\n[[form]]\nfields:\n  characters:\n    type: text\n    width: 80\n[[/form]]", &[]).await;
    let page = import(
        &runner,
        site,
        "long-writing:roster",
        "characters: portrait000\n",
        &[],
    )
    .await;
    let before = read(&runner, site, page).await;
    assert_eq!(
        before.compiled_body_html.unwrap().matches("<img ").count(),
        1
    );
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(page)),
        ..Default::default()
    });
    let edited = run_endpoint!(
        runner,
        page_edit,
        json!({"site_id":site,"page":page,"user_id":ADMIN_USER_ID,
            "last_revision_id":before.revision_id,
            "form_updates":{"characters":characters},
            "revision_comments":"long character list regression",
            "ip_address":common::IP_ADDRESS})
    )
    .expect("form edit must create a revision");
    let after = read(&runner, site, page).await;
    assert_eq!(after.revision_id, edited.revision_id);
    assert_ne!(after.revision_id, before.revision_id);
    let source = after.wikitext.unwrap();
    let values = wikidot_forms::parse_values(&source).unwrap();
    assert_eq!(
        values.get("characters").unwrap().as_str(),
        Some(characters.as_str())
    );
    assert!(
        source.contains(&characters),
        "persisted source lost the exact list: {source}"
    );
    let html = after.compiled_body_html.unwrap();
    assert_eq!(html.matches("<img ").count(), 200, "{html}");
    for (index, name) in names.iter().enumerate() {
        let category = if index % 2 == 0 { "character" } else { "bgc" };
        for attribute in [
            format!("src=\"/-/file/icons/icon_{name}.jpg\""),
            format!("alt=\"{category}:{name}\""),
        ] {
            assert_eq!(html.matches(&attribute).count(), 1, "{attribute}: {html}");
        }
    }
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
    let internal_source = "characters: old\ncontent: old";
    let internal = import(
        &runner,
        site,
        "writing:_settings",
        internal_source,
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
    assert_eq!(
        read(&runner, site, internal).await.wikitext.as_deref(),
        Some(internal_source)
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
async fn rename_rewrites_arc_and_chain_characters_and_writing_references() {
    let (runner, site) = setup().await;
    import(&runner, site, "arc:_template", GROUP_TEMPLATE, &[]).await;
    import(&runner, site, "chain:_template", GROUP_TEMPLATE, &[]).await;
    let arc = import(&runner, site, "arc:saga", "characters: 'old keep'\n", &[]).await;
    let chain = import(&runner, site, "chain:link", "characters: OLD\n", &[]).await;
    let writing = import(
        &runner,
        site,
        "writing:mention",
        "characters: keep\nreferences: 're:keep, RE:OLD re:oldish'\n",
        &["keep", "re:new"],
    )
    .await;
    assert!(
        !read(&runner, site, arc)
            .await
            .compiled_body_html
            .unwrap()
            .contains("icon_new.jpg")
    );

    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"old", "target_tag":"new"})
    );
    let slugs: Vec<&str> = preview.pages.iter().map(|p| p.slug.as_str()).collect();
    assert_eq!(slugs, ["arc:saga", "chain:link"]);
    run_endpoint!(
        runner,
        site_tag_apply,
        json!({"source_tag":"old","target_tag":"new","preview_token":preview.preview_token,"ip_address":common::IP_ADDRESS})
    );
    for (page, expected) in [(arc, "new keep"), (chain, "new")] {
        let after = read(&runner, site, page).await;
        let values =
            wikidot_forms::parse_values(after.wikitext.as_deref().unwrap()).unwrap();
        assert_eq!(values.get("characters").unwrap().as_str(), Some(expected));
        assert!(after.compiled_body_html.unwrap().contains("icon_new.jpg"));
    }

    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"re:old", "target_tag":"re:new"})
    );
    let slugs: Vec<&str> = preview.pages.iter().map(|p| p.slug.as_str()).collect();
    assert_eq!(slugs, ["writing:mention"]);
    run_endpoint!(
        runner,
        site_tag_apply,
        json!({"source_tag":"re:old","target_tag":"re:new","preview_token":preview.preview_token,"ip_address":common::IP_ADDRESS})
    );
    let after = read(&runner, site, writing).await;
    let values = wikidot_forms::parse_values(after.wikitext.as_deref().unwrap()).unwrap();
    assert_eq!(
        values.get("references").unwrap().as_str(),
        Some("re:keep, re:new re:oldish")
    );
    assert_eq!(values.get("characters").unwrap().as_str(), Some("keep"));
    assert_eq!(after.tags, ["keep", "re:new"]);
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
async fn locked_field_only_page_prevents_partial_tag_and_form_changes() {
    let (runner, site) = setup().await;
    let tagged = import(&runner, site, "writing:unlocked", SOURCE, &["old"]).await;
    let field_only = import(&runner, site, "writing:locked", SOURCE, &["new"]).await;
    let tagged_before = read(&runner, site, tagged).await;
    let field_before = read(&runner, site, field_only).await;
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"old","target_tag":"new"})
    );
    run_endpoint!(
        runner,
        page_lock_create,
        json!({"page":field_only,"lock_type":"permission-only","expires_at":null,"reason":null,"override_existing":false,"ip_address":common::IP_ADDRESS})
    );
    let error = run_endpoint_err!(
        runner,
        site_tag_apply,
        json!({"source_tag":"old","target_tag":"new","preview_token":preview.preview_token,"ip_address":common::IP_ADDRESS})
    );
    assert_contains_error!(error, ErrorType::TagPageNotEditable);
    for (id, before) in [(tagged, tagged_before), (field_only, field_before)] {
        let after = read(&runner, site, id).await;
        assert_eq!(after.revision_id, before.revision_id);
        assert_eq!(after.wikitext, before.wikitext);
        assert_eq!(after.tags, before.tags);
    }
    assert!(run_endpoint!(runner, site_tag_history).is_empty());
}

#[tokio::test]
async fn later_field_edit_blocks_undo_without_reverting_other_pages() {
    let (mut runner, site) = setup().await;
    let tagged = import(&runner, site, "writing:undo-tagged", SOURCE, &["old"]).await;
    let field_only = import(&runner, site, "writing:undo-field", SOURCE, &["new"]).await;
    let preview = run_endpoint!(
        runner,
        site_tag_preview,
        json!({"source_tag":"old","target_tag":"new"})
    );
    let applied = run_endpoint!(
        runner,
        site_tag_apply,
        json!({"source_tag":"old","target_tag":"new","preview_token":preview.preview_token,"ip_address":common::IP_ADDRESS})
    );
    let tagged_after = read(&runner, site, tagged).await;
    let before = read(&runner, site, field_only).await;
    runner.set_request_context(RequestContext {
        site_id: Some(site),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(field_only)),
        ..Default::default()
    });
    run_endpoint!(
        runner,
        page_edit,
        json!({"site_id":site,"page":field_only,"user_id":ADMIN_USER_ID,"last_revision_id":before.revision_id,"form_updates":{"content":"Later biography"},"revision_comments":"later field edit","ip_address":common::IP_ADDRESS})
    );
    let later = read(&runner, site, field_only).await;
    let error = run_endpoint_err!(
        runner,
        site_tag_undo,
        json!({"operation_id":applied.operation_id,"ip_address":common::IP_ADDRESS})
    );
    assert_contains_error!(error, ErrorType::TagUndoConflict);
    assert_eq!(
        read(&runner, site, tagged).await.revision_id,
        tagged_after.revision_id
    );
    assert_eq!(
        read(&runner, site, field_only).await.revision_id,
        later.revision_id
    );
    let values = wikidot_forms::parse_values(later.wikitext.as_deref().unwrap()).unwrap();
    assert_eq!(
        values.get("content").unwrap().as_str(),
        Some("Later biography")
    );
    assert_eq!(values.get("characters").unwrap().as_str(), Some(AFTER));
    assert!(!run_endpoint!(runner, site_tag_history)[0].reversed);
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
        json!({"source_tag":"old","target_tag":"new"})
    );
    let applied = run_endpoint!(
        runner,
        site_tag_apply,
        json!({"source_tag":"old","target_tag":"new","preview_token":preview.preview_token,"ip_address":common::IP_ADDRESS})
    );
    assert_eq!(read(&runner, site, numeric).await.tags, ["new"]);
    assert_eq!(
        read(&runner, site, numeric).await.wikitext.as_deref(),
        Some(numeric_source)
    );
    run_endpoint!(
        runner,
        site_tag_undo,
        json!({"operation_id":applied.operation_id,"ip_address":common::IP_ADDRESS})
    );
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
