use super::*;
use crate::models::message_draft::{self, Entity as MessageDraft};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Statement};
use serde_json::{Value, json};

struct Fixture {
    state: ServerState,
    client: ReqwestClient,
    endpoint: String,
    handle: ServerHandle,
    users: [i64; 3],
    record_id: String,
}

impl Fixture {
    async fn new(recipient_type: &str) -> Self {
        let mut config = Config::integration_testing();
        config.job_min_poll_delay = Duration::from_secs(3600);
        config.job_max_poll_delay = Duration::from_secs(3600);
        let state = build_server_state(config, Secrets::load()).await.unwrap();
        let mut users = [0; 3];
        for user_id in &mut users {
            let row = state
                .database
                .query_one_raw(Statement::from_string(
                    sea_orm::DatabaseBackend::Postgres,
                    "INSERT INTO known_user DEFAULT VALUES RETURNING user_id",
                ))
                .await
                .unwrap()
                .unwrap();
            *user_id = row.try_get("", "user_id").unwrap();
            let slug = format!("message-access-{}", cuid2::cuid());
            state.database.execute_raw(Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Postgres,
                "INSERT INTO \"user\" (user_id, name, slug, email, password, name_changes_left, locales) VALUES ($1, $2, $2, $3, 'unused', 2, ARRAY['en'])",
                [(*user_id).into(), slug.clone().into(), format!("{slug}@example.invalid").into()],
            )).await.unwrap();
        }
        let server = rpc_server_builder()
            .set_http_middleware(tower::ServiceBuilder::new().layer(RequestContextLayer))
            .build("127.0.0.1:0")
            .await
            .unwrap();
        let endpoint = format!("http://{}", server.local_addr().unwrap());
        let module = build_module(Arc::clone(&state)).await.unwrap();
        let handle = server.start(module);
        let mut fixture = Self {
            state,
            client: ReqwestClient::new(),
            endpoint,
            handle,
            users,
            record_id: cuid2::cuid(),
        };
        let mut source_params = fixture.params(users[0]);
        source_params["recipients"] = json!([]);
        let recipient_field = match recipient_type {
            "regular" => "recipients",
            "cc" => "carbon_copy",
            "bcc" => "blind_carbon_copy",
            _ => panic!("invalid fixture recipient type"),
        };
        source_params[recipient_field] = json!([users[1]]);
        let original = fixture.rpc("message_draft_create", source_params).await;
        let draft_id = original["result"]["external_id"]
            .as_str()
            .unwrap_or_else(|| panic!("original draft creation failed: {original}"));
        let sent = fixture
            .rpc("message_draft_send", json!({"message_draft_id": draft_id}))
            .await;
        fixture.record_id = sent["result"]["external_id"]
            .as_str()
            .unwrap_or_else(|| panic!("source message send failed: {sent}"))
            .to_owned();
        // Keep an unsent draft as a sentinel for denied-call state preservation.
        let sentinel = fixture.create(users[2], None).await;
        assert!(sentinel.get("error").is_none(), "{sentinel}");
        fixture
    }

    fn params(&self, user_id: i64) -> Value {
        json!({
            "user_id": user_id, "recipients": [self.users[1]],
            "carbon_copy": [], "blind_carbon_copy": [], "locale": "en",
            "subject": "Message access regression", "wikitext": "Preserved message body",
            "reply_to": null, "forwarded_from": null
        })
    }

    async fn create(&self, user_id: i64, reference: Option<(&str, &str)>) -> Value {
        let mut params = self.params(user_id);
        if let Some((field, record_id)) = reference {
            params[field] = json!(record_id);
        }
        self.rpc("message_draft_create", params).await
    }

    async fn rpc(&self, method: &str, params: Value) -> Value {
        self.client
            .post(&self.endpoint)
            .json(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn drafts(&self) -> Vec<message_draft::Model> {
        let mut drafts = MessageDraft::find()
            .filter(message_draft::Column::UserId.is_in(self.users))
            .all(&self.state.database)
            .await
            .unwrap();
        drafts.sort_by(|a, b| a.external_id.cmp(&b.external_id));
        drafts
    }

    async fn assert_allowed(&self, user_id: i64, field: &str) {
        let before = self.drafts().await;
        let response = self.create(user_id, Some((field, &self.record_id))).await;
        assert!(
            response.get("error").is_none(),
            "{field}, user {user_id}: {response}"
        );
        let result = &response["result"];
        assert_eq!(result["user_id"], user_id);
        assert_eq!(result[field], self.record_id);
        assert_eq!(result["subject"], "Message access regression");
        assert_eq!(
            result["recipients"],
            json!({"r": [self.users[1]], "cc": [], "bcc": []})
        );
        let saved = MessageDraft::find_by_id(result["external_id"].as_str().unwrap())
            .one(&self.state.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(serde_json::to_value(saved).unwrap(), *result);
        let after = self.drafts().await;
        assert_eq!(after.len(), before.len() + 1);
        for draft in before {
            assert!(after.contains(&draft), "existing draft changed");
        }
    }

    async fn assert_denied(&self, user_id: i64, field: &str, record_id: &str) {
        let before = self.drafts().await;
        let response = self.create(user_id, Some((field, record_id))).await;
        let after = self.drafts().await;
        assert_eq!(
            after, before,
            "{field}, denied call changed drafts: {response}"
        );
        assert_eq!(response["error"]["code"], 2014, "{field}: {response}");
        assert_eq!(response["error"]["message"], "Message does not exist");
        assert!(response.get("result").is_none(), "{response}");
    }

    async fn stop(self) {
        self.handle.stop().unwrap();
        self.handle.stopped().await;
    }
}

macro_rules! allowed_case {
    ($name:ident, $actor:expr, $field:literal, $category:literal) => {
        #[tokio::test]
        async fn $name() {
            let fixture = Fixture::new($category).await;
            fixture.assert_allowed(fixture.users[$actor], $field).await;
            fixture.stop().await;
        }
    };
}

macro_rules! denied_case {
    ($name:ident, $actor:expr, $field:literal, $missing:expr) => {
        #[tokio::test]
        async fn $name() {
            let fixture = Fixture::new("regular").await;
            let record_id = if $missing {
                cuid2::cuid()
            } else {
                fixture.record_id.clone()
            };
            fixture
                .assert_denied(fixture.users[$actor], $field, &record_id)
                .await;
            fixture.stop().await;
        }
    };
}

allowed_case!(sender_reply, 0, "reply_to", "regular");
allowed_case!(sender_forward, 0, "forwarded_from", "regular");
allowed_case!(recipient_reply, 1, "reply_to", "regular");
allowed_case!(recipient_forward, 1, "forwarded_from", "regular");
allowed_case!(cc_recipient_reply, 1, "reply_to", "cc");
allowed_case!(cc_recipient_forward, 1, "forwarded_from", "cc");
allowed_case!(bcc_recipient_reply, 1, "reply_to", "bcc");
allowed_case!(bcc_recipient_forward, 1, "forwarded_from", "bcc");
denied_case!(outsider_reply_preserves_drafts, 2, "reply_to", false);
denied_case!(
    outsider_forward_preserves_drafts,
    2,
    "forwarded_from",
    false
);
denied_case!(missing_reply_preserves_drafts, 0, "reply_to", true);
denied_case!(missing_forward_preserves_drafts, 0, "forwarded_from", true);
