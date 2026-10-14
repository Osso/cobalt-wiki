//! Native edits committed by one request must appear in subsequent SiteChanges views.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::{ADMIN_USER_ID, SYSTEM_USER_ID};
use deepwell::license::License;
use deepwell::services::permission::PermissionService;
use deepwell::services::role::{
    GrantUserRoleInput, InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::site::{CreateSite, SiteService};
use deepwell::services::view::GetPageViewOutput;
use deepwell::services::{RequestContext, ServiceContext};
use deepwell::types::{Action, Permission, Reference, Resource};
use scraper::{Html, Selector};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};
use serde_json::json;
use uuid::Uuid;

async fn render(runner: &TestRunner, site_id: i64, extra: &str) -> String {
    let tx = runner.state().database.begin().await.unwrap();
    let output = deepwell::endpoints::view::page_view(
        &ServiceContext::new(runner.state(), &tx),
        common::make_params(json!({
            "site_id": site_id, "session_token": null, "locales": ["en"],
            "route": {"slug": "changes:recent", "extra": extra},
        })),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    match output {
        GetPageViewOutput::Found {
            compiled_body_html, ..
        } => compiled_body_html,
        other => panic!("expected owned changes page: {other:?}"),
    }
}

fn rows(html: &str) -> Vec<String> {
    Html::parse_fragment(html)
        .select(
            &Selector::parse(".site-changes-table tbody tr.changes-list-item").unwrap(),
        )
        .map(|row| row.text().collect::<String>())
        .collect()
}

async fn create_owned_site(ctx: &ServiceContext<'_>) -> i64 {
    SiteService::create(
        ctx,
        CreateSite {
            slug: format!("native-changes-{}", Uuid::new_v4().simple()),
            name: "Native changes fixture".into(),
            tagline: String::new(),
            description: "Owned committed SiteChanges regression".into(),
            default_page: None,
            layout: None,
            license: License::CcBySa40,
            locale: "en".into(),
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .site_id
}

async fn create_feed_role(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    name: &str,
    is_virtual: bool,
) -> i64 {
    RoleService::create(
        ctx,
        InternalCreateRoleInput {
            site_id,
            name: name.into(),
            description: None,
            is_virtual,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .role_id
}

async fn update_feed_permissions(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    role_id: i64,
    actions: Vec<Action>,
) {
    let permissions = actions
        .into_iter()
        .map(|action| Permission {
            resource_type: Resource::Page,
            resource_category: None,
            action,
        })
        .collect();
    PermissionService::update_permissions_for_role(
        ctx,
        UpdateRolePermissionsInput {
            site_id,
            role_reference: Reference::Id(role_id),
            new_permissions: permissions,
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
}

async fn grant_feed_permissions(ctx: &ServiceContext<'_>, site_id: i64) {
    let everyone = create_feed_role(ctx, site_id, "everyone", true).await;
    update_feed_permissions(ctx, site_id, everyone, vec![Action::View]).await;
    let editor = create_feed_role(ctx, site_id, "changes-editor", false).await;
    update_feed_permissions(ctx, site_id, editor, vec![Action::Create, Action::Edit])
        .await;
    RoleService::grant_role_to_user(
        ctx,
        GrantUserRoleInput {
            site_id,
            user_id: ADMIN_USER_ID,
            role_id: editor,
            assigning_user_id: SYSTEM_USER_ID,
            expires_at: None,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn committed_native_edits_join_imported_history_in_live_filtered_views() {
    let runner = TestRunner::setup_with_idle_job_workers().await;
    let tx = runner.state().database.begin().await.unwrap();
    let ctx = ServiceContext::new(runner.state(), &tx);
    let site_id = create_owned_site(&ctx).await;
    grant_feed_permissions(&ctx, site_id).await;
    tx.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "INSERT INTO wikidot_site_change
         (site_id, page_slug, page_title, revision_number, flags, changed_at,
          user_slug, user_name, comments)
         SELECT $1, 'writing:archive', 'Imported archive', n, 'S',
                to_timestamp(1790000000 + n * 60), 'allicat', 'Allicat',
                'Preserved imported history' FROM generate_series(0, 24) AS n",
        [site_id.into()],
    ))
    .await
    .unwrap();
    let mut created = Vec::new();
    for (slug, source, comments) in [
        ("changes:recent", "[[module SiteChanges]]", "Feed created"),
        ("writing:native", "Owned native source", "Native creation"),
    ] {
        let actor =
            ServiceContext::new(runner.state(), &tx).with_request(RequestContext {
                site_id: Some(site_id),
                user_id: Some(ADMIN_USER_ID),
                page_reference: Some(Reference::Slug(slug.into())),
                ..Default::default()
            });
        let page = deepwell::endpoints::page::page_create(
            &actor,
            common::make_params(json!({
                "site_id": site_id, "user_id": ADMIN_USER_ID, "slug": slug,
                "title": "Native fixture", "wikitext": source, "layout": "wikidot",
                "revision_comments": comments, "bypass_filter": true,
                "ip_address": common::IP_ADDRESS,
            })),
        )
        .await
        .unwrap();
        created.push(page);
    }
    tx.commit().await.unwrap();
    let before = render(&runner, site_id, "").await;
    let native = &created[1];
    let tx = runner.state().database.begin().await.unwrap();
    let actor = ServiceContext::new(runner.state(), &tx).with_request(RequestContext {
        site_id: Some(site_id),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(native.page_id)),
        ..Default::default()
    });
    let edited = deepwell::endpoints::page::page_edit(
        &actor,
        common::make_params(json!({
            "site_id": site_id, "user_id": ADMIN_USER_ID, "page": native.page_id,
            "last_revision_id": native.revision_id, "tags": ["just-edited"],
            "revision_comments": "Native tags saved after feed was cached",
            "ip_address": common::IP_ADDRESS,
        })),
    )
    .await
    .unwrap()
    .unwrap();
    tx.commit().await.unwrap();
    let persisted = runner
        .state()
        .database
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT changes, from_wikidot FROM page_revision WHERE revision_id = $1",
            [edited.revision_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    let after = render(&runner, site_id, "").await;
    let tagged = render(&runner, site_id, "p/1/category/writing/types/A").await;
    let new = render(&runner, site_id, "p/1/category/writing/types/N").await;
    let tx = runner.state().database.begin().await.unwrap();
    let actor = ServiceContext::new(runner.state(), &tx).with_request(RequestContext {
        site_id: Some(site_id),
        user_id: Some(ADMIN_USER_ID),
        page_reference: Some(Reference::Id(native.page_id)),
        ..Default::default()
    });
    let changed = deepwell::endpoints::page::page_edit(
        &actor,
        common::make_params(json!({
            "site_id": site_id, "user_id": ADMIN_USER_ID, "page": native.page_id,
            "last_revision_id": edited.revision_id,
            "title": "Edited native title", "alt_title": "Native alternate title",
            "wikitext": "Native source changed", "tags": ["second-edit"],
            "revision_comments": "Combined native edit", "ip_address": common::IP_ADDRESS,
        })),
    )
    .await
    .unwrap()
    .unwrap();
    let no_op = deepwell::endpoints::page::page_edit(&actor, common::make_params(json!({
        "site_id": site_id, "user_id": ADMIN_USER_ID, "page": native.page_id,
        "last_revision_id": changed.revision_id, "tags": ["second-edit"],
        "revision_comments": "No revision for unchanged tags", "ip_address": common::IP_ADDRESS,
    }))).await.unwrap();
    tx.commit().await.unwrap();
    let combined = render(
        &runner,
        site_id,
        "p/1/category/writing/perpage/50/types/STM",
    )
    .await;
    let author = runner
        .state()
        .database
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT name FROM \"user\" WHERE user_id = $1",
            [ADMIN_USER_ID.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "name")
        .unwrap();
    let first = render(&runner, site_id, "p/1/category/writing/perpage/20").await;
    let second = render(&runner, site_id, "p/2/category/writing/perpage/20").await;
    let imported_count = runner
        .state()
        .database
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT count(*) AS total FROM wikidot_site_change WHERE site_id = $1",
            [site_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "total")
        .unwrap();
    // Retire only this owned committed site, including on the expected RED assertion.
    runner
        .state()
        .database
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE site SET deleted_at = now() WHERE site_id = $1",
            [site_id.into()],
        ))
        .await
        .unwrap();

    assert_eq!(
        persisted.try_get::<Vec<String>>("", "changes").unwrap(),
        ["tags"]
    );
    assert!(!persisted.try_get::<bool>("", "from_wikidot").unwrap());
    assert!(!before.contains("Native tags saved after feed was cached"));
    assert!(
        after.contains("Native tags saved after feed was cached"),
        "{after}"
    );
    let tag_rows = rows(&tagged);
    assert_eq!(tag_rows.len(), 1, "{tagged}");
    assert!(tag_rows[0].contains("(rev. 1)"));
    assert!(tag_rows[0].contains("Native tags saved after feed was cached"));
    assert!(tagged.contains("title=\"tags changed\">A</span>"));
    assert_eq!(rows(&new).len(), 1, "{new}");
    assert!(new.contains("Native creation"));
    assert!(no_op.is_none());
    let combined_rows = rows(&combined);
    assert_eq!(combined_rows.len(), 26, "{combined}");
    assert!(combined_rows[0].contains("Edited native title"));
    assert!(combined_rows[0].contains("Combined native edit"));
    assert!(combined_rows[0].contains("STAM"));
    assert!(combined_rows[0].contains(&author));
    assert!(!combined.contains("No revision for unchanged tags"));
    assert_eq!(rows(&first).len(), 20);
    assert_eq!(rows(&second).len(), 8);
    assert!(rows(&first)[0].contains("Combined native edit"));
    assert!(rows(&second).last().unwrap().contains("(new)"));
    assert_eq!(imported_count, 25);
    assert_eq!(
        rows(&first)
            .iter()
            .chain(rows(&second).iter())
            .filter(|row| row.contains("Preserved imported history"))
            .count(),
        25
    );
}
