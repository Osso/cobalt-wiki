//! Search returns only pages visible to the request actor in the requested site.

#[path = "common/runner.rs"]
mod runner;

use deepwell::constants::{ADMIN_USER_ID, SYSTEM_USER_ID};
use deepwell::license::License;
use deepwell::services::category::CategoryService;
use deepwell::services::page::{CreatePage, PageService};
use deepwell::services::page_revision::PageRevisionService;
use deepwell::services::permission::PermissionService;
use deepwell::services::role::{
    GrantUserRoleInput, InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use deepwell::services::search::{SearchDocument, SearchRequest, SearchService};
use deepwell::services::site::{CreateSite, SiteService};
use deepwell::services::user::{CreateUser, UserService};
use deepwell::services::{RequestContext, ServiceContext, TextService};
use deepwell::types::{Action, Permission, Reference, Resource, UserType};
use runner::TestRunner;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const IP_ADDRESS: std::net::IpAddr = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);

async fn import_document(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    slug: &str,
    title: &str,
    body: &str,
) -> SearchDocument {
    let created = PageService::import(
        ctx,
        CreatePage {
            site_id,
            slug: slug.into(),
            title: title.into(),
            wikitext: body.into(),
            alt_title: None,
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Search authorization fixture".into(),
            tags: vec![],
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: IP_ADDRESS,
        },
    )
    .await
    .expect("import page");
    let revision = PageRevisionService::get_latest(ctx, site_id, created.page_id)
        .await
        .expect("read revision");
    let html = TextService::get(ctx, &revision.compiled_body_html_hash)
        .await
        .expect("read compiled body");
    let indexed_body = scraper::Html::parse_fragment(&html)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    SearchDocument {
        page_id: created.page_id,
        site_id,
        revision_id: created.revision_id,
        title: revision.title,
        slug: slug.into(),
        tags: revision.tags,
        body: indexed_body,
    }
}

async fn grant_category_view(ctx: &ServiceContext<'_>, site_id: i64, user_id: i64) {
    let category_id = CategoryService::get_or_create(ctx, site_id, "public")
        .await
        .expect("public category")
        .category_id;
    let role_id = RoleService::create(
        ctx,
        InternalCreateRoleInput {
            site_id,
            name: "Search public reader".into(),
            description: None,
            is_virtual: false,
            parent_role_id: None,
            creating_user_id: SYSTEM_USER_ID,
            ip_address: IP_ADDRESS,
        },
    )
    .await
    .expect("create reader role")
    .role_id;
    PermissionService::update_permissions_for_role(
        ctx,
        UpdateRolePermissionsInput {
            site_id,
            role_reference: Reference::Id(role_id),
            new_permissions: vec![Permission {
                resource_type: Resource::Page,
                resource_category: Some(Reference::Id(category_id)),
                action: Action::View,
            }],
            cascade_removals: false,
            updating_user_id: SYSTEM_USER_ID,
            ip_address: IP_ADDRESS,
        },
    )
    .await
    .expect("grant category permission");
    RoleService::grant_role_to_user(
        ctx,
        GrantUserRoleInput {
            site_id,
            user_id,
            role_id,
            assigning_user_id: SYSTEM_USER_ID,
            expires_at: None,
            ip_address: IP_ADDRESS,
        },
    )
    .await
    .expect("assign reader role");
}

async fn create_actor(ctx: &ServiceContext<'_>, label: &str) -> i64 {
    UserService::create(
        ctx,
        CreateUser {
            user_type: UserType::Regular,
            name: format!("Search {label}"),
            email: format!("search-{label}@example.com"),
            locales: vec!["en".into()],
            password: "test-password".into(),
            bypass_filter: true,
            bypass_email_verification: true,
            override_user_id: None,
            ip_address: IP_ADDRESS,
        },
    )
    .await
    .expect("create search actor")
    .user_id
}

async fn serve_hits(listener: TcpListener, documents: Vec<SearchDocument>) {
    let response = json!({ "hits": documents }).to_string();
    for _ in 0..3 {
        let (mut socket, _) = listener.accept().await.expect("accept search request");
        let mut request = [0; 8192];
        let size = socket
            .read(&mut request)
            .await
            .expect("read search request");
        assert!(
            String::from_utf8_lossy(&request[..size])
                .starts_with("POST /indexes/pages/search "),
            "only search requests expected"
        );
        let reply = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.len(),
            response
        );
        socket
            .write_all(reply.as_bytes())
            .await
            .expect("reply with hits");
    }
}

#[tokio::test]
async fn page_search_excludes_denied_and_foreign_hits_from_visible_pagination() {
    let mut runner = TestRunner::setup().await;
    let ctx = runner.context();
    let site_id = SiteService::create(
        ctx,
        CreateSite {
            slug: format!("search-permissions-{}", uuid::Uuid::new_v4().simple()),
            name: "Search permissions".into(),
            tagline: String::new(),
            description: "Search permissions fixture".into(),
            default_page: None,
            layout: None,
            license: License::CcBySa40,
            locale: "en".into(),
            ip_address: IP_ADDRESS,
        },
    )
    .await
    .expect("create requested site")
    .site_id;
    let foreign_site_id = SiteService::create(
        ctx,
        CreateSite {
            slug: format!("search-foreign-{}", uuid::Uuid::new_v4().simple()),
            name: "Foreign search site".into(),
            tagline: String::new(),
            description: "Foreign search fixture".into(),
            default_page: None,
            layout: None,
            license: License::CcBySa40,
            locale: "en".into(),
            ip_address: IP_ADDRESS,
        },
    )
    .await
    .expect("create foreign site")
    .site_id;
    let reader = create_actor(ctx, &format!("reader-{}", uuid::Uuid::new_v4())).await;
    let stranger = create_actor(ctx, &format!("stranger-{}", uuid::Uuid::new_v4())).await;
    grant_category_view(ctx, site_id, reader).await;

    let denied = import_document(
        ctx,
        site_id,
        "secret:denied",
        "Denied title",
        "Denied private snippet",
    )
    .await;
    let foreign = import_document(
        ctx,
        foreign_site_id,
        "public:foreign",
        "Foreign title",
        "Foreign private snippet",
    )
    .await;
    let first = import_document(
        ctx,
        site_id,
        "public:first",
        "First allowed title",
        "First allowed snippet",
    )
    .await;
    let second = import_document(
        ctx,
        site_id,
        "public:second",
        "Second allowed title",
        "Second allowed snippet",
    )
    .await;
    let mut forged_foreign = foreign.clone();
    forged_foreign.site_id = site_id;
    let candidates = vec![
        denied,
        foreign,
        forged_foreign,
        first.clone(),
        second.clone(),
    ];

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake Meili");
    let url = format!("http://{}", listener.local_addr().unwrap());
    let responder = tokio::spawn(serve_hits(listener, candidates));
    let search = SearchService::new(url, "test-key".into());

    runner.set_request_context(RequestContext {
        user_id: Some(reader),
        site_id: Some(site_id),
        ..Default::default()
    });
    for (offset, expected, has_more) in [(0, &first, true), (1, &second, false)] {
        let result = search
            .page_search(
                runner.context(),
                SearchRequest {
                    query: "snippet".into(),
                    offset,
                    limit: 1,
                },
            )
            .await
            .expect("search as category reader");
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].page_id, expected.page_id);
        assert_eq!(result.hits[0].title, expected.title);
        assert_eq!(result.hits[0].snippet, expected.body);
        assert_eq!(result.has_more, has_more);
        let output = serde_json::to_string(&result).unwrap();
        for private in [
            "Denied title",
            "Denied private snippet",
            "Foreign title",
            "Foreign private snippet",
        ] {
            assert!(!output.contains(private), "leaked {private}");
        }
    }

    runner.set_request_context(RequestContext {
        user_id: Some(stranger),
        site_id: Some(site_id),
        page_reference: Some(Reference::Id(first.page_id)),
        ..Default::default()
    });
    let result = search
        .page_search(
            runner.context(),
            SearchRequest {
                query: "snippet".into(),
                offset: 0,
                limit: 1,
            },
        )
        .await
        .expect("search as unprivileged actor");
    assert!(
        result.hits.is_empty(),
        "request page reference must not grant access"
    );
    assert!(
        !result.has_more,
        "denied candidates must not count as pages"
    );
    responder.await.expect("fake Meili completed");
}
