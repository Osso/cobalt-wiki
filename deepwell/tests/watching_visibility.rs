mod common;

use common::TestRunner;
use deepwell::constants::SYSTEM_USER_ID;
use deepwell::license::License;
use deepwell::models::{page, page_revision};
use deepwell::services::TextService;
use deepwell::services::page::{CreatePage, EditPage, EditPageBody, PageService};
use deepwell::services::page_revision::PageRevisionService;
use deepwell::services::permission::PermissionService;
use deepwell::services::relation::{
    CreateSiteMember, RelationService, SiteMemberAccepted, SiteMemberData,
};
use deepwell::services::role::{
    InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::site::{CreateSite, SiteService};
use deepwell::services::user::{CreateUser, UserService};
use deepwell::services::watching::visibility::visible_change;
use deepwell::types::{Action, Maybe, Permission, Reference, Resource, UserType};
use sea_orm::{ActiveModelTrait, Set};

async fn site(runner: &TestRunner) -> i64 {
    let ctx = runner.context();
    let site_id = SiteService::create(
        ctx,
        CreateSite {
            slug: "watch-visibility-site".into(),
            name: "Watch visibility".into(),
            tagline: String::new(),
            description: "Visibility fixture".into(),
            default_page: None,
            layout: None,
            license: License::CcBySa40,
            locale: "en".into(),
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .site_id;
    for role_name in ["member", "anonymous"] {
        let role_id = RoleService::create(
            ctx,
            InternalCreateRoleInput {
                site_id,
                name: role_name.into(),
                description: None,
                is_virtual: true,
                parent_role_id: None,
                creating_user_id: SYSTEM_USER_ID,
                ip_address: common::IP_ADDRESS,
            },
        )
        .await
        .unwrap()
        .role_id;
        PermissionService::update_permissions_for_role(
            ctx,
            UpdateRolePermissionsInput {
                site_id,
                role_reference: Reference::Id(role_id),
                new_permissions: vec![
                    Permission {
                        resource_type: Resource::Site,
                        resource_category: None,
                        action: Action::View,
                    },
                    Permission {
                        resource_type: Resource::Page,
                        resource_category: None,
                        action: Action::View,
                    },
                ],
                cascade_removals: false,
                updating_user_id: SYSTEM_USER_ID,
                ip_address: common::IP_ADDRESS,
            },
        )
        .await
        .unwrap();
    }
    site_id
}

async fn member(runner: &TestRunner, site_id: i64, name: &str) -> i64 {
    let user_id = UserService::create(
        runner.context(),
        CreateUser {
            user_type: UserType::Regular,
            name: name.into(),
            email: format!("watch-{}@example.com", name.replace(' ', "-")),
            locales: vec!["en".into()],
            password: "test-password".into(),
            bypass_filter: true,
            bypass_email_verification: true,
            override_user_id: None,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .user_id;
    RelationService::create_site_member(
        runner.context(),
        CreateSiteMember {
            site_id,
            user_id,
            created_by: SYSTEM_USER_ID,
            metadata: SiteMemberData {
                accepted: SiteMemberAccepted::SelfJoined,
            },
        },
        common::IP_ADDRESS,
    )
    .await
    .unwrap();
    user_id
}

async fn create_page(
    runner: &TestRunner,
    site_id: i64,
    slug: &str,
    source: &str,
) -> (i64, i64) {
    let created = PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            user_id: SYSTEM_USER_ID,
            slug: slug.into(),
            title: slug.into(),
            alt_title: None,
            wikitext: source.into(),
            layout: None,
            revision_comments: "watch fixture".into(),
            tags: vec![],
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    (created.page_id, created.revision_id)
}

async fn edit_page(
    runner: &TestRunner,
    site_id: i64,
    page_id: i64,
    old: i64,
    source: &str,
) -> i64 {
    PageService::edit(
        runner.context(),
        EditPage {
            site_id,
            page: Reference::Id(page_id),
            last_revision_id: old,
            revision_comments: "watch edit".into(),
            user_id: SYSTEM_USER_ID,
            body: EditPageBody {
                wikitext: Maybe::Set(source.into()),
                ..Default::default()
            },
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .unwrap()
    .revision_id
}

const OLD: &str = "Public old.\n[[include :snippets:suo BEGIN CODE |type=showto |user1=watch-a]]\nOld secret A.\n[[include :snippets:suo END CODE]]\n[[include :snippets:suo BEGIN CODE |type=showto |user1=watch-b]]\nOld secret B.\n[[include :snippets:suo END CODE]]";
const NEW: &str = "Public new.\n[[include :snippets:suo BEGIN CODE |type=showto |user1=watch-a]]\nNew secret A.\n[[include :snippets:suo END CODE]]\n[[include :snippets:suo BEGIN CODE |type=showto |user1=watch-b]]\nNew secret B.\n[[include :snippets:suo END CODE]]";

#[tokio::test]
async fn exact_stored_revisions_render_each_viewers_visible_text() {
    let runner = TestRunner::setup().await;
    let ctx = runner.context();
    let site_id = site(&runner).await;
    let a = member(&runner, site_id, "Watch A").await;
    let b = member(&runner, site_id, "Watch B").await;
    let outsider = member(&runner, site_id, "Watch Outsider").await;
    let (page_id, old) = create_page(&runner, site_id, "watch-visible", OLD).await;
    let new = edit_page(&runner, site_id, page_id, old, NEW).await;

    for (user, own_old, own_new, other_old, other_new) in [
        (
            a,
            "Old secret A.",
            "New secret A.",
            "Old secret B.",
            "New secret B.",
        ),
        (
            b,
            "Old secret B.",
            "New secret B.",
            "Old secret A.",
            "New secret A.",
        ),
    ] {
        let change = visible_change(ctx, user, site_id, page_id, Some(old), new)
            .await
            .unwrap()
            .expect("view permitted");
        assert_eq!(change.title, "watch-visible");
        assert_eq!(change.slug, "watch-visible");
        let before = change.before.expect("stored old revision");
        assert!(
            before.contains("Public old.") && before.contains(own_old),
            "{before}"
        );
        assert!(
            change.after.contains("Public new.") && change.after.contains(own_new),
            "{}",
            change.after
        );
        assert!(!before.contains(other_old) && !change.after.contains(other_new));
        assert!(!before.contains("[[include") && !change.after.contains("[[include"));
    }
    let outsider_change = visible_change(ctx, outsider, site_id, page_id, Some(old), new)
        .await
        .unwrap()
        .expect("public page");
    assert!(!outsider_change.before.unwrap().contains("secret"));
    assert!(!outsider_change.after.contains("secret"));
    let creation = visible_change(ctx, a, site_id, page_id, None, new)
        .await
        .unwrap()
        .expect("creation");
    assert_eq!(creation.before, None);
    assert!(creation.after.contains("New secret A."));
}

#[tokio::test]
async fn unavailable_or_hidden_revisions_and_deleted_pages_are_skipped() {
    let runner = TestRunner::setup().await;
    let ctx = runner.context();
    let site_id = site(&runner).await;
    let user = member(&runner, site_id, "Watch A").await;
    let (page_id, old) = create_page(&runner, site_id, "watch-hidden", OLD).await;
    let new = edit_page(&runner, site_id, page_id, old, NEW).await;
    assert!(
        visible_change(ctx, user, site_id, page_id, Some(old), i64::MAX)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        visible_change(ctx, user, site_id, page_id, Some(i64::MAX), new)
            .await
            .unwrap()
            .is_none()
    );
    let revision_request = serde_json::json!({
        "site_id": site_id, "page_id": page_id, "revision_number": 0,
        "details": {"wikitext": true}
    });
    let readable = deepwell::endpoints::all::page_revision_get(
        ctx,
        common::make_params(revision_request.clone()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(readable.wikitext.as_deref(), Some(OLD));
    page_revision::ActiveModel {
        revision_id: Set(old),
        hidden: Set(vec!["wikitext".into()]),
        ..Default::default()
    }
    .update(ctx.transaction())
    .await
    .unwrap();
    assert!(
        visible_change(ctx, user, site_id, page_id, Some(old), new)
            .await
            .unwrap()
            .is_none()
    );
    let hidden = deepwell::endpoints::all::page_revision_get(
        ctx,
        common::make_params(revision_request),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        hidden.wikitext.is_none(),
        "hidden source must also be absent from revision readback"
    );
    page_revision::ActiveModel {
        revision_id: Set(old),
        hidden: Set(vec![]),
        ..Default::default()
    }
    .update(ctx.transaction())
    .await
    .unwrap();
    page_revision::ActiveModel {
        revision_id: Set(new),
        hidden: Set(vec!["title".into()]),
        ..Default::default()
    }
    .update(ctx.transaction())
    .await
    .unwrap();
    assert!(
        visible_change(ctx, user, site_id, page_id, Some(old), new)
            .await
            .unwrap()
            .is_none()
    );
    page::ActiveModel {
        page_id: Set(page_id),
        deleted_at: Set(Some(time::OffsetDateTime::now_utc())),
        ..Default::default()
    }
    .update(ctx.transaction())
    .await
    .unwrap();
    assert!(
        visible_change(ctx, user, site_id, page_id, None, new)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn current_page_view_denial_skips_both_snapshots() {
    use deepwell::models::role_permission;
    use deepwell::types::{Action, Resource};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let runner = TestRunner::setup().await;
    let ctx = runner.context();
    let site_id = site(&runner).await;
    let user = member(&runner, site_id, "Watch A").await;
    let (page_id, old) = create_page(&runner, site_id, "watch-denied", OLD).await;
    let new = edit_page(&runner, site_id, page_id, old, NEW).await;
    assert!(
        visible_change(ctx, user, site_id, page_id, Some(old), new)
            .await
            .unwrap()
            .is_some()
    );
    role_permission::Entity::delete_many()
        .filter(role_permission::Column::SiteId.eq(site_id))
        .filter(role_permission::Column::ResourceType.eq(Resource::Page))
        .filter(role_permission::Column::Action.eq(Action::View))
        .exec(ctx.transaction())
        .await
        .unwrap();
    assert!(
        visible_change(ctx, user, site_id, page_id, Some(old), new)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn included_page_denied_to_shared_view_is_not_in_notification() {
    use deepwell::models::role_permission;
    use deepwell::types::{Action, Resource};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let runner = TestRunner::setup().await;
    let ctx = runner.context();
    let site_id = site(&runner).await;
    let user = member(&runner, site_id, "Watch A").await;
    let (page_id, _) =
        create_page(&runner, site_id, "watch-public:parent", "Public").await;
    create_page(
        &runner,
        site_id,
        "watch-private:secret",
        "Private included secret.",
    )
    .await;
    let parent = PageService::get_direct(ctx, page_id, false).await.unwrap();
    role_permission::Entity::update_many()
        .col_expr(
            role_permission::Column::ResourceCategoryId,
            parent.page_category_id.into(),
        )
        .filter(role_permission::Column::SiteId.eq(site_id))
        .filter(role_permission::Column::ResourceType.eq(Resource::Page))
        .filter(role_permission::Column::Action.eq(Action::View))
        .exec(ctx.transaction())
        .await
        .unwrap();
    let first = PageRevisionService::get_latest(ctx, site_id, page_id)
        .await
        .unwrap()
        .revision_id;
    let new = edit_page(
        &runner,
        site_id,
        page_id,
        first,
        "Parent content.\n[[include watch-private:secret]]",
    )
    .await;
    let change = visible_change(ctx, user, site_id, page_id, Some(first), new)
        .await
        .unwrap()
        .expect("recipient can view parent");
    assert!(change.after.contains("Parent content."), "{}", change.after);
    assert!(
        !change.after.contains("Private included secret."),
        "{}",
        change.after
    );
}

#[tokio::test]
async fn malformed_revision_render_returns_error_without_source_fallback() {
    let runner = TestRunner::setup().await;
    let ctx = runner.context();
    let site_id = site(&runner).await;
    let user = member(&runner, site_id, "Watch A").await;
    let (page_id, revision_id) =
        create_page(&runner, site_id, "watch-cycle", "Initial").await;
    let hash = TextService::create(ctx, "[[include watch-cycle]]".into())
        .await
        .unwrap();
    page_revision::ActiveModel {
        revision_id: Set(revision_id),
        wikitext_hash: Set(hash.to_vec()),
        ..Default::default()
    }
    .update(ctx.transaction())
    .await
    .unwrap();
    let error = visible_change(ctx, user, site_id, page_id, None, revision_id)
        .await
        .expect_err("recursive include must fail closed");
    assert!(
        format!("{error:?}").contains(&format!("revision ID {revision_id}")),
        "{error:?}"
    );
}
