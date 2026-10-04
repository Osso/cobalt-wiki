use super::*;
use crate::constants::SYSTEM_USER_ID;
use crate::models::{page, page_revision};
use crate::services::permission::PermissionService;
use crate::services::role::{
    GrantUserRoleInput, InternalCreateRoleInput, RoleService, UpdateRolePermissionsInput,
};
use crate::services::session::CreateSession;
use crate::types::{Action, Permission, Reference, Resource};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter, QueryOrder,
    Statement,
};
use serde_json::{Value, json};

struct Fixture {
    state: ServerState,
    handle: ServerHandle,
    endpoint: String,
    site_id: i64,
    category_id: i64,
    editor_id: i64,
    editor: String,
    restricted: String,
    outsider: String,
    slug: String,
}

#[derive(Debug, PartialEq, Eq)]
struct DeletedRecordsSnapshot {
    pages: Vec<page::Model>,
    revisions: Vec<page_revision::Model>,
}

impl Fixture {
    async fn new() -> Self {
        let mut config = Config::integration_testing();
        config.session_token_length = 64;
        config.job_min_poll_delay = Duration::from_secs(3600);
        config.job_max_poll_delay = Duration::from_secs(3600);
        let state = build_server_state(config, Secrets::load()).await.unwrap();
        let txn = state.database.begin().await.unwrap();
        let ctx = ServiceContext::new(&state, &txn);
        let suffix = cuid2::cuid();
        let site_id = insert_id(&txn, &format!(
            "INSERT INTO site (slug,name,tagline,description,locale,license) VALUES ('deleted-{suffix}','Fixture','','','en','cc-by-sa') RETURNING site_id"
        ), "site_id").await;
        let category_id = insert_id(&txn, &format!(
            "INSERT INTO page_category (site_id,slug) VALUES ({site_id},'restore') RETURNING category_id"
        ), "category_id").await;
        let mut users = Vec::new();
        for _ in 0..2 {
            let user_id = insert_id(
                &txn,
                "INSERT INTO known_user DEFAULT VALUES RETURNING user_id",
                "user_id",
            )
            .await;
            let slug = format!("deleted-user-{}", cuid2::cuid());
            txn.execute_unprepared(&format!(
                "INSERT INTO \"user\" (user_id,name,slug,email,password,name_changes_left,locales) VALUES ({user_id},'{slug}','{slug}','{slug}@example.invalid','unused',2,ARRAY['en'])"
            )).await.unwrap();
            users.push(user_id);
        }
        let ip_address = "127.0.0.1".parse().unwrap();
        let role = RoleService::create(
            &ctx,
            InternalCreateRoleInput {
                site_id,
                name: format!("Deleted editor {suffix}"),
                description: None,
                is_virtual: false,
                parent_role_id: None,
                creating_user_id: SYSTEM_USER_ID,
                ip_address,
            },
        )
        .await
        .unwrap();
        PermissionService::update_permissions_for_role(
            &ctx,
            UpdateRolePermissionsInput {
                site_id,
                role_reference: Reference::Id(role.role_id),
                new_permissions: vec![Permission {
                    resource_type: Resource::Page,
                    resource_category: Some(Reference::Id(category_id)),
                    action: Action::Edit,
                }],
                cascade_removals: false,
                updating_user_id: SYSTEM_USER_ID,
                ip_address,
            },
        )
        .await
        .unwrap();
        RoleService::grant_role_to_user(
            &ctx,
            GrantUserRoleInput {
                site_id,
                user_id: users[0],
                role_id: role.role_id,
                assigning_user_id: users[0],
                expires_at: None,
                ip_address,
            },
        )
        .await
        .unwrap();
        let mut tokens = Vec::new();
        for (user_id, restricted) in
            [(users[0], false), (users[0], true), (users[1], false)]
        {
            tokens.push(
                SessionService::create(
                    &ctx,
                    CreateSession {
                        user_id,
                        ip_address,
                        user_agent: "deleted-page-access".into(),
                        restricted,
                    },
                )
                .await
                .unwrap(),
            );
        }
        txn.commit().await.unwrap();
        let server = rpc_server_builder()
            .set_http_middleware(tower::ServiceBuilder::new().layer(RequestContextLayer))
            .build("127.0.0.1:0")
            .await
            .unwrap();
        let endpoint = format!("http://{}", server.local_addr().unwrap());
        let handle = server.start(build_module(Arc::clone(&state)).await.unwrap());
        Self {
            state,
            handle,
            endpoint,
            site_id,
            category_id,
            editor_id: users[0],
            editor: tokens[0].clone(),
            restricted: tokens[1].clone(),
            outsider: tokens[2].clone(),
            slug: format!("restore:deleted-{suffix}"),
        }
    }

    async fn add_deleted(&self, category_id: i64) -> i64 {
        let txn = self.state.database.begin().await.unwrap();
        let page_id = insert_id(&txn, &format!(
            "INSERT INTO page (site_id,page_category_id,slug,deleted_at) VALUES ({},{category_id},'{}',clock_timestamp()) RETURNING page_id",
            self.site_id, self.slug
        ), "page_id").await;
        txn.execute_unprepared("INSERT INTO text (hash,contents) VALUES (decode('00112233445566778899aabbccddeeff','hex'),'deleted fixture') ON CONFLICT DO NOTHING").await.unwrap();
        let revision_id = insert_id(&txn, &format!(
            "INSERT INTO page_revision (revision_type,revision_number,page_id,site_id,user_id,changes,wikitext_hash,compiled_body_html_hash,compiled_at,compiled_generator,comments,title,slug,tags) VALUES ('delete',1,{page_id},{},{}, '{{}}',decode('00112233445566778899aabbccddeeff','hex'),decode('00112233445566778899aabbccddeeff','hex'),now(),'fixture','deleted fixture','Deleted secret title','{}',ARRAY['private-tag']) RETURNING revision_id",
            self.site_id, self.editor_id, self.slug
        ), "revision_id").await;
        txn.execute_unprepared(&format!(
            "UPDATE page SET latest_revision_id={revision_id} WHERE page_id={page_id}"
        ))
        .await
        .unwrap();
        txn.commit().await.unwrap();
        page_id
    }

    async fn load_deleted_records_snapshot(&self) -> DeletedRecordsSnapshot {
        let pages = page::Entity::find()
            .filter(page::Column::SiteId.eq(self.site_id))
            .order_by_asc(page::Column::PageId)
            .all(&self.state.database)
            .await
            .unwrap();
        let revisions = page_revision::Entity::find()
            .filter(page_revision::Column::SiteId.eq(self.site_id))
            .order_by_asc(page_revision::Column::RevisionId)
            .all(&self.state.database)
            .await
            .unwrap();
        DeletedRecordsSnapshot { pages, revisions }
    }

    async fn lookup(
        &self,
        token: Option<&str>,
        header_site: i64,
        header_slug: &str,
        site_id: i64,
        slug: &str,
    ) -> Value {
        let before = self.load_deleted_records_snapshot().await;
        let mut request = ReqwestClient::new()
            .post(&self.endpoint)
            .header("X-Deepwell-Site-Id", header_site.to_string())
            .header("X-Deepwell-Page", header_slug);
        if let Some(token) = token {
            request = request.header("X-Deepwell-Session-Token", token);
        }
        let response = request.json(&json!({"jsonrpc":"2.0","id":1,"method":"page_get_deleted","params":{"site_id":site_id,"slug":slug}}))
            .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
        let after = self.load_deleted_records_snapshot().await;
        assert_eq!(
            before, after,
            "lookup changed fixture-owned pages or revisions"
        );
        response
    }

    async fn denied(
        &self,
        token: Option<&str>,
        header_site: i64,
        header_slug: &str,
        site_id: i64,
        slug: &str,
    ) {
        let response = self
            .lookup(token, header_site, header_slug, site_id, slug)
            .await;
        assert_eq!(response["error"]["code"], 3106, "{response}");
        assert!(response.get("result").is_none(), "{response}");
        assert!(
            !response.to_string().contains("Deleted secret title"),
            "{response}"
        );
        assert!(!response.to_string().contains("private-tag"), "{response}");
    }
}

async fn insert_id(txn: &sea_orm::DatabaseTransaction, sql: &str, column: &str) -> i64 {
    txn.query_one_raw(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .unwrap()
        .unwrap()
        .try_get("", column)
        .unwrap()
}

#[tokio::test]
async fn deleted_page_lookup_uses_restore_permissions_and_trusted_http_identity() {
    let fixture = Fixture::new().await;
    let first = fixture.add_deleted(fixture.category_id).await;
    let second = fixture.add_deleted(fixture.category_id).await;
    let site = fixture.site_id;
    let slug = &fixture.slug;
    let editor = Some(fixture.editor.as_str());
    for token in [
        None,
        Some(fixture.restricted.as_str()),
        Some(fixture.outsider.as_str()),
    ] {
        fixture.denied(token, site, slug, site, slug).await;
    }
    // The body target is editable; a different trusted route cannot select it.
    let other_page = fixture.add_deleted(fixture.category_id).await;
    let other_slug = format!("restore:other-{}", cuid2::cuid());
    let txn = fixture.state.database.begin().await.unwrap();
    txn.execute_unprepared(&format!(
        "UPDATE page SET slug='{other_slug}' WHERE page_id={other_page}; UPDATE page_revision SET slug='{other_slug}' WHERE page_id={other_page}"
    )).await.unwrap();
    let foreign_site = insert_id(&txn, &format!(
        "INSERT INTO site (slug,name,tagline,description,locale,license) VALUES ('foreign-{}','Fixture','','','en','cc-by-sa') RETURNING site_id", cuid2::cuid()
    ), "site_id").await;
    txn.commit().await.unwrap();
    assert!(
        fixture
            .lookup(editor, site, &other_slug, site, &other_slug)
            .await["result"]
            .is_array()
    );
    fixture.denied(editor, foreign_site, slug, site, slug).await;
    fixture.denied(editor, site, &other_slug, site, slug).await;
    let response = fixture.lookup(editor, site, slug, site, slug).await;
    let records = response["result"]
        .as_array()
        .unwrap_or_else(|| panic!("{response}"));
    assert_eq!(records.len(), 2);
    for page_id in [first, second] {
        let record = records
            .iter()
            .find(|record| record["page_id"] == page_id)
            .unwrap();
        assert_eq!(record["title"], "Deleted secret title");
        assert_eq!(record["tags"], json!(["private-tag"]));
        assert_eq!(record["slug"], *slug);
    }
    assert_eq!(
        fixture
            .lookup(editor, site, "missing", site, "missing")
            .await["result"],
        json!([])
    );
    fixture.state.database.execute_unprepared(&format!(
        "INSERT INTO page_lock (lock_type,page_id,user_id,reason) VALUES ('wikidot',{first},{},'Restore lock')", fixture.editor_id
    )).await.unwrap();
    fixture.denied(editor, site, slug, site, slug).await;
    fixture
        .state
        .database
        .execute_unprepared(&format!("DELETE FROM page_lock WHERE page_id={first}"))
        .await
        .unwrap();
    // Same deleted slug, different actual category: all candidates must be authorized.
    let txn = fixture.state.database.begin().await.unwrap();
    let protected = insert_id(&txn, &format!("INSERT INTO page_category (site_id,slug) VALUES ({site},'protected') RETURNING category_id"), "category_id").await;
    txn.commit().await.unwrap();
    fixture.add_deleted(protected).await;
    fixture.denied(editor, site, slug, site, slug).await;
    fixture.handle.stop().unwrap();
    fixture.handle.stopped().await;
}
