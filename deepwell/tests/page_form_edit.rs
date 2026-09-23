//! JSON-RPC endpoint behavior; requires the seeded DB, cache and S3 test services.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::ErrorType;
use deepwell::services::RequestContext;
use deepwell::services::page::{CreatePageOutput, GetPageOutput};
use deepwell::types::Reference;
use serde_json::{Value, json};

const SLUG: &str = "form-edit:example";
const TEMPLATE: &str = "[[form]]\nfields:\n  name:\n    type: text\n  count:\n    type: text\n  notes:\n    type: wiki\n  kind:\n    type: select\n    values:\n      a: Alpha\n      b: Beta\n[[/form]]";
const SOURCE: &str = "name: Before\ncount: 3\nnotes: old\nkind: a\nunknown: true\n";

async fn setup() -> (TestRunner, i64) {
    let mut runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .expect("Seeded site not found")
        .site
        .site_id;
    runner.set_request_context(RequestContext {
        session: None,
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug(SLUG.into())),
    });
    (runner, site_id)
}

async fn create_page(
    runner: &mut TestRunner,
    site_id: i64,
    slug: &str,
    source: &str,
) -> CreatePageOutput {
    runner.set_request_context(RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug(slug.to_owned().into())),
        ..Default::default()
    });
    run_endpoint!(
        runner,
        page_create,
        json!({
            "site_id": site_id, "slug": slug, "title": "Form edit fixture",
            "wikitext": source, "revision_comments": "Create fixture",
            "user_id": ADMIN_USER_ID, "ip_address": common::IP_ADDRESS
        })
    )
}

fn edit_request(site_id: i64, page_id: i64, revision_id: i64, fields: Value) -> Value {
    let mut request = json!({
        "site_id": site_id, "page": page_id, "last_revision_id": revision_id,
        "revision_comments": "Edit fixture", "user_id": ADMIN_USER_ID,
        "ip_address": common::IP_ADDRESS
    });
    request
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    request
}

async fn read_page(runner: &TestRunner, site_id: i64, page_id: i64) -> GetPageOutput {
    run_endpoint!(
        runner,
        page_get,
        json!({
            "site_id": site_id, "page": page_id, "details": {"wikitext": true}
        })
    )
    .expect("Fixture missing")
}

#[tokio::test]
async fn scalar_updates_round_trip_and_stale_edits_leave_the_newer_revision_untouched() {
    let (mut runner, site_id) = setup().await;
    create_page(&mut runner, site_id, "form-edit:_template", TEMPLATE).await;
    let page = create_page(&mut runner, site_id, SLUG, SOURCE).await;
    let edited = run_endpoint!(
        runner,
        page_edit,
        edit_request(
            site_id,
            page.page_id,
            page.revision_id,
            json!({
                "form_updates": {"name": "After", "count": 9, "notes": null, "kind": "b"},
                "title": "Updated title"
            })
        )
    )
    .expect("Expected form revision");
    let stored = read_page(&runner, site_id, page.page_id).await;
    assert_eq!(stored.revision_id, edited.revision_id);
    assert_eq!(stored.revision_user_id, ADMIN_USER_ID);
    assert_eq!(stored.title, "Updated title");
    let values =
        wikidot_forms::parse_values(stored.wikitext.as_deref().unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(values).unwrap(),
        json!({
            "name": "After", "count": 9, "notes": null, "kind": "b", "unknown": true
        })
    );

    // Raw edits remain explicit and can store non-form source. A stale form edit
    // must fail on revision identity before attempting to parse that newer source.
    let newer_source = "not: [valid YAML";
    let newer = run_endpoint!(
        runner,
        page_edit,
        edit_request(
            site_id,
            page.page_id,
            edited.revision_id,
            json!({
                "wikitext": newer_source
            })
        )
    )
    .expect("Expected raw revision");
    let error = run_endpoint_err!(
        runner,
        page_edit,
        edit_request(
            site_id,
            page.page_id,
            edited.revision_id,
            json!({
                "form_updates": {"name": "Stale"}
            })
        )
    );
    assert_contains_error!(error, ErrorType::NotLatestRevisionId);
    assert_no_error!(error, ErrorType::BadRequest);
    let stored = read_page(&runner, site_id, page.page_id).await;
    assert_eq!(stored.revision_id, newer.revision_id);
    assert_eq!(stored.wikitext.as_deref(), Some(newer_source));
}

#[tokio::test]
async fn invalid_updates_conflicting_modes_and_templates_do_not_create_revisions() {
    let (mut runner, site_id) = setup().await;
    let template =
        create_page(&mut runner, site_id, "form-edit:_template", TEMPLATE).await;
    let page = create_page(&mut runner, site_id, SLUG, SOURCE).await;
    for fields in [
        json!({"wikitext": "", "form_updates": {}}),
        json!({"form_updates": {"unknown": false}}),
        json!({"form_updates": {"kind": "invalid"}}),
        json!({"form_updates": {"name": ["nested"]}}),
    ] {
        let error = run_endpoint_err!(
            runner,
            page_edit,
            edit_request(site_id, page.page_id, page.revision_id, fields)
        );
        assert_contains_error!(error, ErrorType::BadRequest);
        let stored = read_page(&runner, site_id, page.page_id).await;
        assert_eq!(stored.revision_id, page.revision_id);
        assert_eq!(stored.wikitext.as_deref(), Some(SOURCE));
    }
    runner.set_request_context(RequestContext {
        session: None,
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Id(template.page_id)),
    });
    let error = run_endpoint_err!(
        runner,
        page_edit,
        edit_request(
            site_id,
            template.page_id,
            template.revision_id,
            json!({
                "form_updates": {"name": "Must not edit template as YAML"}
            })
        )
    );
    assert_contains_error!(error, ErrorType::BadRequest);
    let stored = read_page(&runner, site_id, template.page_id).await;
    assert_eq!(stored.revision_id, template.revision_id);
    assert_eq!(stored.wikitext.as_deref(), Some(TEMPLATE));
}

#[tokio::test]
async fn request_context_permission_precedes_form_validation_and_does_not_trust_body_user()
 {
    let (mut runner, site_id) = setup().await;
    create_page(&mut runner, site_id, "form-edit:_template", "[[form]]").await;
    let page = create_page(&mut runner, site_id, SLUG, "not: [yaml").await;
    runner.set_request_context(RequestContext {
        session: None,
        user_id: None,
        site_id: Some(site_id),
        page_reference: Some(Reference::Id(page.page_id)),
    });
    for fields in [
        json!({"form_updates": {"name": "No access"}}),
        json!({"wikitext": "", "form_updates": {}}),
    ] {
        let error = run_endpoint_err!(
            runner,
            page_edit,
            edit_request(site_id, page.page_id, page.revision_id, fields)
        );
        assert_contains_error!(error, ErrorType::PermissionDenied);
        assert_no_error!(error, ErrorType::BadRequest);
    }
    let stored = read_page(&runner, site_id, page.page_id).await;
    assert_eq!(stored.revision_id, page.revision_id);
    assert_eq!(stored.wikitext.as_deref(), Some("not: [yaml"));
}
