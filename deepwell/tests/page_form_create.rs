//! Creating a page in a data-form category stores the record Wikidot's form saves.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::ErrorType;
use deepwell::services::page::CreatePage;
use deepwell::services::{PageService, RequestContext};
use deepwell::types::Reference;
use serde_json::json;

// Shape of the archived character:_template form.
const TEMPLATE: &str = "%%form_data{name}%%\n====\n[[form]]
fields:
  header-player:
    type: static
    label: Player
    value: Your OOC player name/handle.
  name:
    label: Name
    type: text
  aka:
    label: AKA
    type: text
    default: '@@'
  sex:
    label: Body Type
    type: select
    values:
      female: Female
      male: Male
[[/form]]";

#[tokio::test]
async fn form_category_pages_are_created_from_the_category_form() {
    let mut runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            slug: "formcreate:_template".into(),
            title: "Template".into(),
            wikitext: TEMPLATE.into(),
            alt_title: None,
            tags: vec![],
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Form template fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();

    // Anonymous visitors get neither permission nor the form.
    runner.set_request_context(RequestContext {
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug("formcreate:isabeau".into())),
        ..Default::default()
    });
    let anonymous = run_endpoint!(runner, page_create_permission);
    assert!(!anonymous.can_create && anonymous.form.is_none());

    runner.set_request_context(RequestContext {
        user_id: Some(ADMIN_USER_ID),
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug("formcreate:isabeau".into())),
        ..Default::default()
    });
    let permission = run_endpoint!(runner, page_create_permission);
    assert!(permission.can_create);
    let form = permission.form.expect("category form for the new page");
    let fields: Vec<_> = form["schema"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["name"].as_str().unwrap())
        .collect();
    assert_eq!(fields, ["header-player", "name", "aka", "sex"]);
    assert_eq!(form["values"], json!({}));

    let payload = |extra: serde_json::Value| {
        let mut payload = json!({
            "site_id": site_id, "slug": "formcreate:isabeau", "title": "Isabeau Clark",
            "wikitext": "", "revision_comments": "", "user_id": ADMIN_USER_ID,
            "ip_address": common::IP_ADDRESS,
        });
        payload
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        payload
    };
    let rejected = run_endpoint_err!(
        runner,
        page_create,
        payload(json!({"form_updates": {"sex": "other"}}))
    );
    assert_contains_error!(rejected, ErrorType::BadRequest);

    run_endpoint!(
        runner,
        page_create,
        payload(json!({"form_updates": {"name": "Isabeau Clark", "sex": "female"}}))
    );
    let page = run_endpoint!(
        runner,
        page_get,
        json!({"site_id": site_id, "page": "formcreate:isabeau", "details": {"wikitext": true}})
    )
    .expect("created page");
    assert_eq!(
        page.wikitext.as_deref(),
        Some("name: 'Isabeau Clark'\naka: '@@'\nsex: female"),
    );
}
