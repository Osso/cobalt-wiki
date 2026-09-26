//! Site member administration: only the site's admins (or root) may list
//! members with their emails, change roles, remove and invite members. The
//! acting user is the request's session user, never a parameter.

#[macro_use]
mod common;

use common::{TestRunner, fake_mailgun, form_field, wait_for_requests};
use deepwell::config::Config;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::prelude::*;
use deepwell::models::session::Model as SessionModel;
use deepwell::models::user_role;
use deepwell::services::RequestContext;
use deepwell::services::member_admin::{MemberRole, SiteMemberEntry};
use deepwell::services::permission::PermissionService;
use deepwell::services::relation::{
    CreatePageAttribution, PageAttributionKind, PageAttributionMetadata,
};
use deepwell::services::role::UpdateRolePermissionsInput;
use deepwell::services::{PageLockService, RelationService};
use deepwell::types::{Action, Permission, Resource};
use deepwell::types::{PageLockType, Reference};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::{Value, json};
use time::macros::datetime;

struct Site {
    site_id: i64,
    /// Role name -> role ID.
    roles: Vec<(String, i64)>,
}

impl Site {
    async fn load(runner: &TestRunner) -> Self {
        let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
            .unwrap()
            .site
            .site_id;
        let roles = run_endpoint!(runner, list_site_roles, json!({"site_id": site_id}))
            .into_iter()
            .map(|role| (role.name, role.role_id))
            .collect();
        Site { site_id, roles }
    }

    fn role_id(&self, name: &str) -> i64 {
        self.roles
            .iter()
            .find(|(role, _)| role == name)
            .unwrap_or_else(|| panic!("test site has a {name} role"))
            .1
    }
}

/// A site member since 2021-04-29 holding `member` plus the given roles.
async fn add_member(
    runner: &TestRunner,
    site: &Site,
    name: &str,
    extra_roles: &[&str],
) -> i64 {
    let user_id = run_endpoint!(
        runner,
        user_create,
        json!({
            "user_type": "regular", "name": name,
            "email": format!("{}@example.com", name.to_lowercase()),
            "locales": ["en"], "password": "secret",
            "bypass_filter": true, "bypass_email_verification": true,
            "ip_address": common::IP_ADDRESS,
        }),
    )
    .user_id;
    run_endpoint!(
        runner,
        membership_set,
        json!({
            "site_id": site.site_id, "user_id": user_id,
            "metadata": {"accepted": {"cause": "accepted", "user_id": ADMIN_USER_ID}},
            "created_by": ADMIN_USER_ID,
            "joined_at": "2021-04-29T14:55:00Z",
            "ip_address": common::IP_ADDRESS,
        }),
    );
    for role in std::iter::once(&"member").chain(extra_roles) {
        run_endpoint!(
            runner,
            grant_role_to_user,
            json!({
                "user_id": user_id, "role_id": site.role_id(role), "site_id": site.site_id,
                "assigning_user_id": ADMIN_USER_ID, "expires_at": null,
                "ip_address": common::IP_ADDRESS,
            }),
        );
    }
    user_id
}

fn act_as(runner: &mut TestRunner, site: &Site, user_id: Option<i64>) {
    runner.set_request_context(RequestContext {
        user_id,
        site_id: Some(site.site_id),
        ..Default::default()
    });
}

/// Non-virtual role names the user holds, sorted.
async fn role_names(runner: &TestRunner, site: &Site, user_id: i64) -> Vec<String> {
    let mut names: Vec<String> = run_endpoint!(
        runner,
        get_user_roles,
        json!({"site_id": site.site_id, "user_id": user_id}),
    )
    .into_iter()
    .filter(|role| !role.is_virtual)
    .map(|role| role.name)
    .collect();
    names.sort();
    names
}

async fn block_page(runner: &TestRunner, site: &Site, slug: &str) -> i64 {
    let page_id = run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id": site.site_id, "user_id": ADMIN_USER_ID, "slug": slug,
            "title": "Block test", "wikitext": "Original body", "alt_title": null,
            "layout": "wikidot", "revision_comments": "Imported", "bypass_filter": true,
            "ip_address": common::IP_ADDRESS,
        })
    )
    .page_id;
    RelationService::create_page_attribution(
        runner.context(),
        CreatePageAttribution {
            page_id,
            user_id: ADMIN_USER_ID,
            created_by: ADMIN_USER_ID,
            metadata: PageAttributionMetadata {
                attribution_type: PageAttributionKind::Author,
                attribution_date: time::OffsetDateTime::now_utc().date(),
            },
        },
    )
    .await
    .unwrap();
    page_id
}

fn lock_input(page: i64, lock_type: &str, override_existing: bool) -> Value {
    json!({
        "page": page, "lock_type": lock_type, "expires_at": null,
        "reason": null, "override_existing": override_existing,
        "ip_address": common::IP_ADDRESS,
    })
}

async fn verify_native_lock_and_permission_bypass(
    runner: &mut TestRunner,
    site: &Site,
    page: i64,
    member: i64,
    moderator: i64,
) {
    let reference = Reference::Id(page);
    act_as(runner, site, Some(moderator));
    run_endpoint!(
        runner,
        page_lock_create,
        lock_input(page, "author-or-permission-only", false)
    );
    let denied = PageLockService::set_wikidot_block(
        runner.context(),
        reference.borrow(),
        true,
        common::IP_ADDRESS,
    )
    .await
    .unwrap_err();
    assert_contains_error!(denied, ErrorType::PageLockExists);
    assert!(
        !PageLockService::is_wikidot_blocked(runner.context(), reference.borrow())
            .await
            .unwrap()
    );
    assert_eq!(
        PageLockService::get_locks_for_page(
            runner.context(),
            site.site_id,
            reference.borrow()
        )
        .await
        .unwrap()[0]
            .lock_type,
        PageLockType::AuthorOrPermissionOnly
    );
    let denied = run_endpoint_err!(
        runner,
        page_lock_remove,
        json!({"page": page, "ip_address": common::IP_ADDRESS})
    );
    assert_contains_error!(denied, ErrorType::PermissionDenied);
    let denied = run_endpoint_err!(
        runner,
        page_lock_create,
        lock_input(page, "permission-only", true)
    );
    assert_contains_error!(denied, ErrorType::PermissionDenied);
    act_as(runner, site, Some(ADMIN_USER_ID));
    let author = PageLockService::can_user_bypass_lock(
        runner.context(),
        site.site_id,
        page,
        ADMIN_USER_ID,
    )
    .await
    .unwrap();
    assert!(author.lock_present && author.can_edit);
    run_endpoint!(
        runner,
        page_lock_remove,
        json!({"page": page, "ip_address": common::IP_ADDRESS})
    );
    run_endpoint!(
        runner,
        page_lock_create,
        lock_input(page, "permission-only", false)
    );
    let native = PageLockService::can_user_bypass_lock(
        runner.context(),
        site.site_id,
        page,
        ADMIN_USER_ID,
    )
    .await
    .unwrap();
    assert!(native.lock_present && !native.can_edit);

    let member_role = site.role_id("member");
    let mut permissions: Vec<_> =
        PermissionService::permissions_as_set(runner.context(), member_role)
            .await
            .unwrap()
            .into_iter()
            .collect();
    permissions.push(Permission {
        resource_type: Resource::Page,
        resource_category: None,
        action: Action::BypassLock,
    });
    PermissionService::update_permissions_for_role(
        runner.context(),
        UpdateRolePermissionsInput {
            site_id: site.site_id,
            role_reference: Reference::Id(member_role),
            new_permissions: permissions,
            cascade_removals: false,
            updating_user_id: ADMIN_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    act_as(runner, site, Some(member));
    let allowed = PageLockService::can_user_bypass_lock(
        runner.context(),
        site.site_id,
        page,
        member,
    )
    .await
    .unwrap();
    assert!(allowed.lock_present && allowed.can_edit);
    run_endpoint!(
        runner,
        page_lock_remove,
        json!({"page": page, "ip_address": common::IP_ADDRESS})
    );
    act_as(runner, site, Some(ADMIN_USER_ID));
    PageLockService::set_wikidot_block(
        runner.context(),
        reference.borrow(),
        true,
        common::IP_ADDRESS,
    )
    .await
    .unwrap();
    act_as(runner, site, Some(member));
    let denied = PageLockService::can_user_bypass_lock(
        runner.context(),
        site.site_id,
        page,
        member,
    )
    .await
    .unwrap();
    assert!(denied.lock_present && !denied.can_edit);
    let denied = run_endpoint_err!(
        runner,
        page_lock_remove,
        json!({"page": page, "ip_address": common::IP_ADDRESS})
    );
    assert_contains_error!(denied, ErrorType::PermissionDenied);
}

#[tokio::test]
async fn wikidot_block_requires_moderator_grant_and_preserves_native_locks() {
    let mut runner = TestRunner::setup().await;
    let site = Site::load(&runner).await;
    let page = block_page(&runner, &site, "block-toggle-policy").await;
    let member = add_member(&runner, &site, "BlockMember", &[]).await;
    let moderator = add_member(&runner, &site, "BlockModerator", &["moderator"]).await;
    let reference = Reference::Id(page);

    act_as(&mut runner, &site, Some(member));
    let denied = PageLockService::set_wikidot_block(
        runner.context(),
        reference.borrow(),
        true,
        common::IP_ADDRESS,
    )
    .await
    .unwrap_err();
    assert_contains_error!(denied, ErrorType::PermissionDenied);
    assert!(
        !PageLockService::is_wikidot_blocked(runner.context(), reference.borrow())
            .await
            .unwrap()
    );

    act_as(&mut runner, &site, Some(moderator));
    PageLockService::set_wikidot_block(
        runner.context(),
        reference.borrow(),
        true,
        common::IP_ADDRESS,
    )
    .await
    .unwrap();
    assert!(
        PageLockService::is_wikidot_blocked(runner.context(), reference.borrow())
            .await
            .unwrap()
    );
    let result = PageLockService::can_user_bypass_lock(
        runner.context(),
        site.site_id,
        page,
        moderator,
    )
    .await
    .unwrap();
    assert!(result.lock_present && result.can_edit);

    act_as(&mut runner, &site, Some(member));
    let result = PageLockService::can_user_bypass_lock(
        runner.context(),
        site.site_id,
        page,
        member,
    )
    .await
    .unwrap();
    assert!(result.lock_present && !result.can_edit);
    let denied = run_endpoint_err!(
        runner,
        page_lock_remove,
        json!({"page": page, "ip_address": common::IP_ADDRESS})
    );
    assert_contains_error!(denied, ErrorType::PermissionDenied);
    let denied = run_endpoint_err!(
        runner,
        page_lock_create,
        lock_input(page, "permission-only", true)
    );
    assert_contains_error!(denied, ErrorType::PermissionDenied);
    assert!(
        PageLockService::is_wikidot_blocked(runner.context(), reference.borrow())
            .await
            .unwrap()
    );

    act_as(&mut runner, &site, Some(moderator));
    PageLockService::set_wikidot_block(
        runner.context(),
        reference.borrow(),
        false,
        common::IP_ADDRESS,
    )
    .await
    .unwrap();
    assert!(
        !PageLockService::is_wikidot_blocked(runner.context(), reference.borrow())
            .await
            .unwrap()
    );
    let result = PageLockService::can_user_bypass_lock(
        runner.context(),
        site.site_id,
        page,
        moderator,
    )
    .await
    .unwrap();
    assert!(!result.lock_present && result.can_edit);

    verify_native_lock_and_permission_bypass(&mut runner, &site, page, member, moderator)
        .await;
}

#[tokio::test]
async fn tombstoned_page_keeps_its_block_policy_for_restore() {
    let mut runner = TestRunner::setup().await;
    let site = Site::load(&runner).await;
    let locked = block_page(&runner, &site, "block-deleted-restore").await;
    let plain = block_page(&runner, &site, "unlocked-deleted-restore").await;
    let editor = add_member(&runner, &site, "BlockRestoreEditor", &[]).await;
    act_as(&mut runner, &site, Some(ADMIN_USER_ID));
    PageLockService::set_wikidot_block(
        runner.context(),
        Reference::Id(locked),
        true,
        common::IP_ADDRESS,
    )
    .await
    .unwrap();
    for page_id in [locked, plain] {
        let page = run_endpoint!(
            runner,
            page_get,
            json!({"site_id": site.site_id, "page": page_id})
        )
        .unwrap();
        run_endpoint!(
            runner,
            page_delete,
            json!({
                "site_id": site.site_id, "page": page_id,
                "last_revision_id": page.revision_id, "revision_comments": "Archive",
                "user_id": ADMIN_USER_ID, "ip_address": common::IP_ADDRESS,
            })
        );
    }
    act_as(&mut runner, &site, Some(editor));
    let locked_state = PageLockService::can_user_bypass_lock(
        runner.context(),
        site.site_id,
        locked,
        editor,
    )
    .await
    .unwrap();
    assert!(locked_state.lock_present && !locked_state.can_edit);
    let plain_state = PageLockService::can_user_bypass_lock(
        runner.context(),
        site.site_id,
        plain,
        editor,
    )
    .await
    .unwrap();
    assert!(!plain_state.lock_present && plain_state.can_edit);
}

#[tokio::test]
async fn expired_moderator_grant_cannot_manage_or_bypass_block() {
    let mut runner = TestRunner::setup().await;
    let site = Site::load(&runner).await;
    let page = block_page(&runner, &site, "block-expired-moderator").await;
    let user = add_member(&runner, &site, "BlockExpiredMod", &["moderator"]).await;
    user_role::ActiveModel {
        user_id: Set(user),
        role_id: Set(site.role_id("moderator")),
        site_id: Set(site.site_id),
        expires_at: Set(Some(datetime!(2020-01-01 00:00 UTC))),
        ..Default::default()
    }
    .update(runner.context().transaction())
    .await
    .unwrap();
    act_as(&mut runner, &site, Some(ADMIN_USER_ID));
    PageLockService::set_wikidot_block(
        runner.context(),
        Reference::Id(page),
        true,
        common::IP_ADDRESS,
    )
    .await
    .unwrap();
    act_as(&mut runner, &site, Some(user));
    let state =
        PageLockService::can_user_bypass_lock(runner.context(), site.site_id, page, user)
            .await
            .unwrap();
    assert!(state.lock_present && !state.can_edit);
    let denied = PageLockService::set_wikidot_block(
        runner.context(),
        Reference::Id(page),
        false,
        common::IP_ADDRESS,
    )
    .await
    .unwrap_err();
    assert_contains_error!(denied, ErrorType::PermissionDenied);
    assert!(
        PageLockService::is_wikidot_blocked(runner.context(), Reference::Id(page))
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn blocked_editor_cannot_delete_body_page_with_forged_unlocked_header() {
    let mut runner = TestRunner::setup().await;
    let site = Site::load(&runner).await;
    let locked_page = block_page(&runner, &site, "block-delete-body-target").await;
    let unlocked_page = block_page(&runner, &site, "block-delete-header-decoy").await;
    let editor = add_member(&runner, &site, "BlockDeleteEditor", &[]).await;
    act_as(&mut runner, &site, Some(ADMIN_USER_ID));
    PageLockService::set_wikidot_block(
        runner.context(),
        Reference::Id(locked_page),
        true,
        common::IP_ADDRESS,
    )
    .await
    .unwrap();
    let before = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site.site_id, "page": locked_page})
    )
    .unwrap();
    runner.set_request_context(RequestContext {
        user_id: Some(editor),
        site_id: Some(site.site_id),
        page_reference: Some(Reference::Id(unlocked_page)),
        ..Default::default()
    });
    let error = run_endpoint_err!(
        runner,
        page_delete,
        json!({
            "site_id": site.site_id, "page": locked_page,
            "last_revision_id": before.revision_id,
            "revision_comments": "Should be blocked", "user_id": editor,
            "ip_address": common::IP_ADDRESS,
        })
    );
    assert_contains_error!(error, ErrorType::PermissionDenied);
    let after = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site.site_id, "page": locked_page})
    )
    .unwrap();
    assert_eq!(after.revision_id, before.revision_id);
    assert!(
        PageLockService::is_wikidot_blocked(runner.context(), Reference::Id(locked_page))
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn lock_endpoint_rejects_wrong_site_and_unprivileged_wikidot_create() {
    let mut runner = TestRunner::setup().await;
    let site = Site::load(&runner).await;
    let page = block_page(&runner, &site, "block-endpoint-binding").await;
    let member = add_member(&runner, &site, "BlockEndpointMember", &[]).await;
    act_as(&mut runner, &site, Some(member));
    let denied =
        run_endpoint_err!(runner, page_lock_create, lock_input(page, "wikidot", false));
    assert_contains_error!(denied, ErrorType::PermissionDenied);
    assert!(
        PageLockService::get_locks_for_page(
            runner.context(),
            site.site_id,
            Reference::Id(page)
        )
        .await
        .unwrap()
        .is_empty()
    );

    let non_editor = run_endpoint!(runner, user_create, json!({
        "user_type": "regular", "name": "BlockNonEditor", "email": "block-non-editor@example.com",
        "locales": ["en"], "password": "secret", "bypass_filter": true,
        "bypass_email_verification": true, "ip_address": common::IP_ADDRESS,
    })).user_id;
    act_as(&mut runner, &site, Some(non_editor));
    let denied = run_endpoint_err!(
        runner,
        page_lock_create,
        lock_input(page, "permission-only", false)
    );
    assert_contains_error!(denied, ErrorType::PermissionDenied);
    assert!(
        PageLockService::get_locks_for_page(
            runner.context(),
            site.site_id,
            Reference::Id(page)
        )
        .await
        .unwrap()
        .is_empty()
    );

    act_as(&mut runner, &site, Some(ADMIN_USER_ID));
    let wrong_site = RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site.site_id + 1000),
        ..Default::default()
    };
    runner.set_request_context(wrong_site);
    assert!(
        deepwell::endpoints::all::page_lock_create(
            runner.context(),
            common::make_params(lock_input(page, "wikidot", false))
        )
        .await
        .is_err()
    );
    assert!(
        deepwell::endpoints::all::page_lock_remove(
            runner.context(),
            common::make_params(json!({"page": page, "ip_address": common::IP_ADDRESS}))
        )
        .await
        .is_err()
    );
    runner.set_request_context(RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site.site_id),
        ..Default::default()
    });
    assert!(
        PageLockService::get_locks_for_page(
            runner.context(),
            site.site_id,
            Reference::Id(page)
        )
        .await
        .unwrap()
        .is_empty()
    );
}

async fn is_member(runner: &TestRunner, site: &Site, user_id: i64) -> bool {
    run_endpoint!(
        runner,
        membership_get,
        json!({"site_id": site.site_id, "user_id": user_id}),
    )
    .is_some()
}

async fn list(runner: &TestRunner) -> Vec<SiteMemberEntry> {
    run_endpoint!(runner, member_admin_list, json!({}))
}

fn set_role(user_id: i64, role: &str) -> Value {
    json!({"user_id": user_id, "role": role, "ip_address": common::IP_ADDRESS})
}

fn remove(user_id: i64) -> Value {
    json!({"user_id": user_id, "ip_address": common::IP_ADDRESS})
}

#[tokio::test]
async fn non_admins_are_refused_and_change_nothing() {
    let mut runner = TestRunner::setup().await;
    let site = Site::load(&runner).await;
    let member = add_member(&runner, &site, "MaPlain", &[]).await;
    let moderator = add_member(&runner, &site, "MaModerator", &["moderator"]).await;
    let target = add_member(&runner, &site, "MaTarget", &[]).await;

    let restricted_admin = RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site.site_id),
        session: Some(SessionModel {
            session_token: "restricted-test-session".into(),
            user_id: ADMIN_USER_ID,
            created_at: datetime!(2026-09-24 00:00 UTC),
            expires_at: datetime!(2099-01-01 00:00 UTC),
            ip_address: common::IP_ADDRESS.to_string(),
            user_agent: "test".into(),
            restricted: true,
        }),
        ..Default::default()
    };
    let callers = [
        ("anonymous", None),
        ("member", Some(member)),
        ("moderator", Some(moderator)),
    ];

    for (who, caller) in callers {
        act_as(&mut runner, &site, caller);
        check_refused(&runner, who, target).await;
    }
    runner.set_request_context(restricted_admin);
    check_refused(&runner, "restricted admin session", target).await;

    act_as(&mut runner, &site, Some(ADMIN_USER_ID));
    assert!(is_member(&runner, &site, target).await);
    assert_eq!(role_names(&runner, &site, target).await, ["member"]);
    assert!(
        !list(&runner)
            .await
            .iter()
            .any(|entry| entry.email == "ma-invitee@example.com"),
        "no refused invite created a member"
    );
}

async fn check_refused(runner: &TestRunner, who: &str, target: i64) {
    let errors = [
        run_endpoint_err!(runner, member_admin_list, json!({})),
        run_endpoint_err!(runner, member_admin_set_role, set_role(target, "admin")),
        run_endpoint_err!(runner, member_admin_remove, remove(target)),
        run_endpoint_err!(
            runner,
            member_admin_invite,
            json!({
                "email": "ma-invitee@example.com", "name": "MaInvitee",
                "ip_address": common::IP_ADDRESS,
            }),
        ),
    ];
    for error in errors {
        assert!(
            extract_error!(error, ErrorType::PermissionDenied).is_some(),
            "{who} was not refused: {error:?}"
        );
    }
}

#[tokio::test]
async fn admin_lists_members_with_email_join_date_and_highest_role() {
    let mut runner = TestRunner::setup().await;
    let site = Site::load(&runner).await;
    let plain = add_member(&runner, &site, "MaListPlain", &[]).await;
    let moderator = add_member(&runner, &site, "MaListMod", &["moderator"]).await;
    let root = add_member(&runner, &site, "MaListRoot", &["admin", "root"]).await;
    act_as(&mut runner, &site, Some(ADMIN_USER_ID));

    let entries = list(&runner).await;
    let find = |user_id: i64| {
        entries
            .iter()
            .find(|entry| entry.user_id == user_id)
            .expect("member listed")
    };
    assert_eq!(
        *find(plain),
        SiteMemberEntry {
            user_id: plain,
            name: "MaListPlain".into(),
            slug: "malistplain".into(),
            email: "malistplain@example.com".into(),
            joined_at: datetime!(2021-04-29 14:55:00 UTC),
            role: MemberRole::Member,
        }
    );
    assert_eq!(find(moderator).role, MemberRole::Moderator);
    assert_eq!(find(root).role, MemberRole::Root);
    assert_eq!(
        serde_json::to_value(find(plain)).unwrap()["joined_at"],
        "2021-04-29T14:55:00Z"
    );
}

#[tokio::test]
async fn admin_promotes_demotes_and_promotes_again() {
    let mut runner = TestRunner::setup().await;
    let site = Site::load(&runner).await;
    let target = add_member(&runner, &site, "MaRoles", &[]).await;
    act_as(&mut runner, &site, Some(ADMIN_USER_ID));

    let steps = [
        ("moderator", vec!["member", "moderator"]),
        ("admin", vec!["admin", "member"]),
        ("member", vec!["member"]),
        // Revoked grants keep their row; granting again revives it.
        ("moderator", vec!["member", "moderator"]),
        ("admin", vec!["admin", "member"]),
    ];
    for (role, expected) in steps {
        run_endpoint!(runner, member_admin_set_role, set_role(target, role));
        assert_eq!(
            role_names(&runner, &site, target).await,
            expected,
            "after {role}"
        );
    }
    let entry = list(&runner)
        .await
        .into_iter()
        .find(|entry| entry.user_id == target)
        .unwrap();
    assert_eq!(entry.role, MemberRole::Admin);

    let error =
        run_endpoint_err!(runner, member_admin_set_role, set_role(target, "root"));
    assert_contains_error!(error, ErrorType::BadRequest);
    assert_eq!(
        role_names(&runner, &site, target).await,
        ["admin", "member"]
    );
}

#[tokio::test]
async fn admin_removes_members_but_not_root_or_themselves() {
    let mut runner = TestRunner::setup().await;
    let site = Site::load(&runner).await;
    let target = add_member(&runner, &site, "MaRemoved", &["moderator"]).await;
    let root = add_member(&runner, &site, "MaRoot", &["admin", "root"]).await;
    act_as(&mut runner, &site, Some(ADMIN_USER_ID));

    run_endpoint!(runner, member_admin_remove, remove(target));
    assert!(!is_member(&runner, &site, target).await);
    assert!(role_names(&runner, &site, target).await.is_empty());
    assert!(!list(&runner).await.iter().any(|e| e.user_id == target));

    // No longer a member: neither removable nor promotable.
    run_endpoint_err!(runner, member_admin_remove, remove(target));
    run_endpoint_err!(runner, member_admin_set_role, set_role(target, "admin"));
    assert!(role_names(&runner, &site, target).await.is_empty());

    for params in [remove(root), set_role(root, "member")] {
        let error = if params.get("role").is_some() {
            run_endpoint_err!(runner, member_admin_set_role, params)
        } else {
            run_endpoint_err!(runner, member_admin_remove, params)
        };
        assert_contains_error!(error, ErrorType::RootMemberProtected);
    }
    assert!(is_member(&runner, &site, root).await);
    assert_eq!(
        role_names(&runner, &site, root).await,
        ["admin", "member", "root"]
    );

    // A root member acts as an admin.
    act_as(&mut runner, &site, Some(root));
    let own = run_endpoint_err!(runner, member_admin_remove, remove(root));
    assert_contains_error!(own, ErrorType::OwnMembership);
    assert!(list(&runner).await.iter().any(|e| e.user_id == root));
}

#[tokio::test]
async fn invite_creates_account_membership_and_emails_a_working_link() {
    let (sender, requests) = fake_mailgun().await;
    let mut runner = TestRunner::setup_with_mailgun(sender).await;
    let site = Site::load(&runner).await;
    let existing = add_member(&runner, &site, "MaExisting", &[]).await;
    act_as(&mut runner, &site, Some(ADMIN_USER_ID));
    run_endpoint!(runner, member_admin_remove, remove(existing));

    let invite = |email: &str, name: Option<&str>| json!({"email": email, "name": name, "ip_address": common::IP_ADDRESS});

    // Unknown address without a name: nothing to create.
    let error = run_endpoint_err!(
        runner,
        member_admin_invite,
        invite("ma-new@example.com", None)
    );
    assert_contains_error!(error, ErrorType::UserNameRequired);

    let created = run_endpoint!(
        runner,
        member_admin_invite,
        invite(" MA-New@Example.com ", Some("MaNewcomer")),
    );
    assert!(created.created && created.emailed);
    assert!(is_member(&runner, &site, created.user_id).await);
    assert_eq!(
        role_names(&runner, &site, created.user_id).await,
        ["member"]
    );
    let entry = list(&runner)
        .await
        .into_iter()
        .find(|entry| entry.user_id == created.user_id)
        .unwrap();
    assert_eq!(
        (entry.name.as_str(), entry.email.as_str()),
        ("MaNewcomer", "MA-New@Example.com")
    );

    let sent = wait_for_requests(&requests, 1).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(form_field(&sent[0], "to"), "MA-New@Example.com");
    assert_eq!(
        form_field(&sent[0], "subject"),
        "Choose your password for Test site"
    );
    let token = form_field(&sent[0], "text")
        .split_once("https://test.wikijump.com/-/set-password/")
        .expect("link in email")
        .1
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    run_endpoint!(
        runner,
        password_token_redeem,
        json!({"token": token, "password": "newcomer secret", "ip_address": common::IP_ADDRESS}),
    );

    // An account that exists (not a member any more) joins without an email.
    let rejoined = run_endpoint!(
        runner,
        member_admin_invite,
        invite("maexisting@example.com", None),
    );
    assert_eq!(
        serde_json::to_value(&rejoined).unwrap(),
        json!({"user_id": existing, "created": false, "emailed": false}),
    );
    assert!(is_member(&runner, &site, existing).await);
    assert_eq!(role_names(&runner, &site, existing).await, ["member"]);

    // Inviting a current member is refused and keeps their join date.
    let error = run_endpoint_err!(
        runner,
        member_admin_invite,
        invite("ma-new@example.com", Some("MaNewcomer")),
    );
    assert_contains_error!(error, ErrorType::SiteMemberExists);
    assert_eq!(wait_for_requests(&requests, 2).await.len(), 1);
}

#[tokio::test]
async fn invite_of_a_new_account_needs_mailgun() {
    let mut runner = TestRunner::setup().await;
    let site = Site::load(&runner).await;
    act_as(&mut runner, &site, Some(ADMIN_USER_ID));

    let error = run_endpoint_err!(
        runner,
        member_admin_invite,
        json!({
            "email": "ma-nomail@example.com", "name": "MaNoMail",
            "ip_address": common::IP_ADDRESS,
        }),
    );
    assert_contains_error!(error, ErrorType::EmailSend);
}

#[tokio::test]
async fn preload_tells_the_header_whether_the_session_user_is_a_site_admin() {
    // The test configuration's 16-character tokens fail the session table's
    // length check, so sign-in needs real-length tokens here.
    let mut config = Config::integration_testing();
    config.session_token_length = 64;
    let runner = TestRunner::setup_with_config(config).await;
    let site = Site::load(&runner).await;
    add_member(&runner, &site, "MaHeaderAdmin", &["admin"]).await;
    add_member(&runner, &site, "MaHeaderMod", &["moderator"]).await;

    let preload = |session_token: Option<String>| json!({"site_id": site.site_id, "locales": ["en"], "session_token": session_token});
    for (name, expected) in [("maheaderadmin", true), ("maheadermod", false)] {
        let login = run_endpoint!(
            runner,
            auth_login,
            json!({
                "name_or_email": name, "password": "secret",
                "ip_address": common::IP_ADDRESS, "user_agent": "test",
            }),
        );
        let output =
            run_endpoint!(runner, preload_view, preload(Some(login.session_token)));
        assert_eq!(output.site_admin, expected, "{name}");
    }
    let anonymous = run_endpoint!(runner, preload_view, preload(None));
    assert!(!anonymous.site_admin);
}
