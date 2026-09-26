//! Move repairs only selected, editable dependency pages and reports references left behind.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::{SAMPLE_USER_ID, SYSTEM_USER_ID};
use deepwell::models::{page_connection, page_revision};
use deepwell::services::PageLockService;
use deepwell::services::RequestContext;
use deepwell::services::page::{CreatePage, PageService};
use deepwell::services::page_lock::CreatePageLockInput;
use deepwell::services::permission::PermissionService;
use deepwell::services::role::{
    GrantUserRoleInput, InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::types::{Action, PageLockType, Permission, Reference, Resource};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Value, json};

async fn import_page(runner: &TestRunner, site_id: i64, slug: &str, source: &str) -> i64 {
    PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            user_id: SYSTEM_USER_ID,
            slug: slug.into(),
            title: slug.into(),
            alt_title: None,
            wikitext: source.into(),
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Fixture".into(),
            tags: vec![],
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .page_id
}

async fn grant_view_and_edit(
    runner: &TestRunner,
    site_id: i64,
    view_pages: &[i64],
    edit_pages: &[i64],
) {
    let role = RoleService::create(
        runner.context(),
        InternalCreateRoleInput {
            site_id,
            name: "Move dependency editor".into(),
            description: None,
            is_virtual: false,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    let mut permissions = Vec::new();
    for (pages, action) in [(view_pages, Action::View), (edit_pages, Action::Edit)] {
        for page_id in pages {
            let category_id =
                PageService::get(runner.context(), site_id, Reference::Id(*page_id))
                    .await
                    .unwrap()
                    .page_category_id;
            permissions.push(Permission {
                resource_type: Resource::Page,
                resource_category: Some(Reference::Id(category_id)),
                action,
            });
        }
    }
    PermissionService::update_permissions_for_role(
        runner.context(),
        UpdateRolePermissionsInput {
            site_id,
            role_reference: Reference::Id(role.role_id),
            new_permissions: permissions,
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    RoleService::grant_role_to_user(
        runner.context(),
        GrantUserRoleInput {
            site_id,
            user_id: SAMPLE_USER_ID,
            role_id: role.role_id,
            assigning_user_id: SYSTEM_USER_ID,
            expires_at: None,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
}

async fn fixture() -> (TestRunner, i64, i64) {
    let mut runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let target = import_page(&runner, site_id, "old-page", "Target").await;
    let editable = import_page(&runner, site_id, "editable:anchor", "Fixture").await;
    let protected = import_page(&runner, site_id, "protected:anchor", "Fixture").await;
    grant_view_and_edit(
        &runner,
        site_id,
        &[target, editable, protected],
        &[target, editable],
    )
    .await;
    runner.set_request_context(RequestContext {
        user_id: Some(SAMPLE_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Id(target)),
        ..Default::default()
    });
    (runner, site_id, target)
}

async fn revisions(runner: &TestRunner, page_id: i64) -> Vec<page_revision::Model> {
    page_revision::Entity::find()
        .filter(page_revision::Column::PageId.eq(page_id))
        .all(runner.context().transaction())
        .await
        .unwrap()
}

async fn source(runner: &TestRunner, site_id: i64, page_id: i64) -> String {
    run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": page_id, "details": {"wikitext": true}})
    )
    .unwrap()
    .wikitext
    .unwrap()
}

async fn move_input(
    runner: &TestRunner,
    site_id: i64,
    target: i64,
    selected: &[i64],
) -> Value {
    let revision = PageService::get(runner.context(), site_id, Reference::Id(target))
        .await
        .unwrap()
        .latest_revision_id
        .unwrap();
    json!({
        "site_id": site_id,
        "page": target,
        "new_slug": "new-page",
        "last_revision_id": revision,
        "revision_comments": "Move target",
        "user_id": SAMPLE_USER_ID,
        "ip_address": common::IP_ADDRESS,
        "fix_dependencies": selected,
    })
}

async fn connections(runner: &TestRunner, source_id: i64) -> Vec<page_connection::Model> {
    page_connection::Entity::find()
        .filter(page_connection::Column::FromPageId.eq(source_id))
        .all(runner.context().transaction())
        .await
        .unwrap()
}

#[tokio::test]
async fn selected_link_and_include_repair_once_and_leave_unselected_and_denied_sources() {
    let (runner, site_id, target) = fixture().await;
    let original = "[[[old-page|Label]]] [[[ OLD PAGE  | spaced label ]]] [[[old-page]]]\n[[include Old Page]]\n[[[other-page|old-page]]]";
    let updated = "[[[new-page|Label]]] [[[new-page  | spaced label ]]] [[[new-page]]]\n[[include new-page]]\n[[[other-page|old-page]]]";
    let selected = import_page(&runner, site_id, "editable:selected", &original).await;
    let unselected =
        import_page(&runner, site_id, "editable:unselected", "[[[old-page]]]").await;
    let denied =
        import_page(&runner, site_id, "protected:denied", "[[include old-page]]").await;
    let before_unselected = revisions(&runner, unselected).await;
    let before_denied = revisions(&runner, denied).await;

    let result = serde_json::to_value(run_endpoint!(
        runner,
        page_move,
        move_input(&runner, site_id, target, &[selected, selected, denied]).await
    ))
    .unwrap();
    assert_eq!(result["repaired_dependencies"], json!([selected]));
    assert_eq!(
        result["remaining_dependencies"],
        json!({
            "links": [{"page_id": unselected, "slug": "editable:unselected", "title": "editable:unselected"}],
            "inclusions": [{"page_id": denied, "slug": "protected:denied", "title": "protected:denied"}],
        })
    );
    assert_eq!(source(&runner, site_id, selected).await, updated);
    let history = revisions(&runner, selected).await;
    assert_eq!(
        history.len(),
        2,
        "duplicate selection must create one revision"
    );
    assert_eq!(history[1].user_id, SAMPLE_USER_ID);
    assert!(
        !history[1].comments.is_empty(),
        "repair needs automatic comment"
    );
    assert_eq!(source(&runner, site_id, unselected).await, "[[[old-page]]]");
    assert_eq!(
        source(&runner, site_id, denied).await,
        "[[include old-page]]"
    );
    assert_eq!(revisions(&runner, unselected).await, before_unselected);
    assert_eq!(revisions(&runner, denied).await, before_denied);
    assert!(
        connections(&runner, selected)
            .await
            .iter()
            .any(|connection| connection.to_page_id == target)
    );
    assert_eq!(result["new_slug"], "new-page");
}

#[tokio::test]
async fn active_lock_and_non_candidate_ids_do_not_mutate_sources() {
    let (runner, site_id, target) = fixture().await;
    let locked = import_page(&runner, site_id, "editable:locked", "[[[old-page]]]").await;
    let unrelated =
        import_page(&runner, site_id, "editable:unrelated", "Unrelated").await;
    let foreign_site = run_endpoint!(runner, site_get, json!({"site": "scp-wiki"}))
        .unwrap()
        .site
        .site_id;
    let foreign =
        import_page(&runner, foreign_site, "foreign-source", "[[[old-page]]]").await;
    PageLockService::create(
        runner.context(),
        site_id,
        SAMPLE_USER_ID,
        Reference::Id(locked),
        CreatePageLockInput {
            page: Reference::Id(locked),
            expires_at: None,
            from_wikidot: false,
            lock_type: PageLockType::PermissionOnly,
            reason: Some("Fixture lock".into()),
            override_existing: false,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    let before = revisions(&runner, locked).await;
    let output = serde_json::to_value(run_endpoint!(
        runner,
        page_move,
        move_input(
            &runner,
            site_id,
            target,
            &[locked, unrelated, foreign, i64::MAX]
        )
        .await
    ))
    .unwrap();
    assert_eq!(output["repaired_dependencies"], json!([]));
    assert_eq!(
        output["remaining_dependencies"],
        json!({
            "links": [{"page_id": locked, "slug": "editable:locked", "title": "editable:locked"}],
            "inclusions": [],
        })
    );
    assert_eq!(source(&runner, site_id, locked).await, "[[[old-page]]]");
    assert_eq!(revisions(&runner, locked).await, before);
    assert_eq!(revisions(&runner, unrelated).await.len(), 1);
    assert_eq!(revisions(&runner, foreign).await.len(), 1);
}

#[tokio::test]
async fn unsupported_include_and_near_match_leave_selected_page_and_history_unchanged() {
    let (runner, site_id, target) = fixture().await;
    let original = "[[include category:old-page]]\n[[include old-page |arg=x]]\n[[[old-page-extra]]]";
    let selected = import_page(&runner, site_id, "editable:unsupported", original).await;
    let before = revisions(&runner, selected).await;
    let output = serde_json::to_value(run_endpoint!(
        runner,
        page_move,
        move_input(&runner, site_id, target, &[selected]).await
    ))
    .unwrap();
    assert_eq!(output["repaired_dependencies"], json!([]));
    assert_eq!(source(&runner, site_id, selected).await, original);
    assert_eq!(revisions(&runner, selected).await, before);
}

#[tokio::test]
async fn omitted_selection_defaults_to_no_repairs() {
    let (runner, site_id, target) = fixture().await;
    let source_id =
        import_page(&runner, site_id, "editable:default", "[[[old-page]]]").await;
    let mut input = move_input(&runner, site_id, target, &[]).await;
    input.as_object_mut().unwrap().remove("fix_dependencies");
    let output = serde_json::to_value(run_endpoint!(runner, page_move, input)).unwrap();
    assert_eq!(output["repaired_dependencies"], json!([]));
    assert_eq!(
        output["remaining_dependencies"],
        json!({
            "links": [{"page_id": source_id, "slug": "editable:default", "title": "editable:default"}],
            "inclusions": [],
        })
    );
    assert_eq!(source(&runner, site_id, source_id).await, "[[[old-page]]]");
    assert_eq!(revisions(&runner, source_id).await.len(), 1);
}
