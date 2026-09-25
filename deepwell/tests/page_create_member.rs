mod common;

use common::TestRunner;
use deepwell::constants::SYSTEM_USER_ID;
use deepwell::license::License;
use deepwell::services::RequestContext;
use deepwell::services::permission::PermissionService;
use deepwell::services::relation::{
    CreateSiteMember, RelationService, SiteMemberAccepted, SiteMemberData,
};
use deepwell::services::role::{
    GetUserRolesInput, InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::site::{CreateSite, SiteService};
use deepwell::services::user::{CreateUser, UserService};
use deepwell::types::{Action, Permission, Reference, Resource, UserType};
use serde_json::json;

#[tokio::test]
async fn member_can_create_missing_page_without_becoming_its_author() {
    let mut runner = TestRunner::setup().await;
    let ctx = runner.context();
    let site = SiteService::create(
        ctx,
        CreateSite {
            slug: "member-create-site".into(),
            name: "Member create site".into(),
            tagline: String::new(),
            description: "Member creation test site".into(),
            default_page: None,
            layout: None,
            license: License::CcBySa40,
            locale: "en".into(),
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    let member_role = RoleService::create(
        ctx,
        InternalCreateRoleInput {
            site_id: site.site_id,
            name: "member".into(),
            description: None,
            is_virtual: true,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    RoleService::create(
        ctx,
        InternalCreateRoleInput {
            site_id: site.site_id,
            name: "page-author".into(),
            description: None,
            is_virtual: true,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    let permissions = vec![
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
        Permission {
            resource_type: Resource::Page,
            resource_category: None,
            action: Action::Create,
        },
    ];
    PermissionService::update_permissions_for_role(
        ctx,
        UpdateRolePermissionsInput {
            site_id: site.site_id,
            role_reference: Reference::Id(member_role.role_id),
            new_permissions: permissions,
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    let user = UserService::create(
        ctx,
        CreateUser {
            user_type: UserType::Regular,
            name: "member-create-user".into(),
            email: "member-create-user@example.com".into(),
            locales: vec!["en".into()],
            password: "test-password".into(),
            bypass_filter: true,
            bypass_email_verification: true,
            override_user_id: None,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
    RelationService::create_site_member(
        ctx,
        CreateSiteMember {
            site_id: site.site_id,
            user_id: user.user_id,
            created_by: SYSTEM_USER_ID,
            metadata: SiteMemberData {
                accepted: SiteMemberAccepted::SelfJoined,
            },
        },
        common::IP_ADDRESS,
    )
    .await
    .unwrap();

    let page_reference = Reference::Slug("story:member-created".into());
    runner.set_request_context(RequestContext {
        user_id: Some(user.user_id),
        site_id: Some(site.site_id),
        page_reference: Some(page_reference.clone()),
        ..Default::default()
    });
    let roles = RoleService::get_virtual_roles_for_user(
        runner.context(),
        &GetUserRolesInput {
            site_id: site.site_id,
            user_id: Some(user.user_id),
            page_reference: Some(page_reference),
        },
    )
    .await
    .unwrap();
    assert!(roles.iter().any(|role| role.name == "member"));
    assert!(!roles.iter().any(|role| role.name == "page-author"));

    let created = deepwell::endpoints::all::page_create(runner.context(), common::make_params(json!({
        "site_id": site.site_id, "user_id": user.user_id, "slug": "story:member-created",
        "title": "Member created", "wikitext": "Visible text", "revision_comments": "created",
        "tags": [], "bypass_filter": true, "ip_address": common::IP_ADDRESS,
    }))).await.unwrap();
    assert_eq!(created.slug, "story:member-created");
}
