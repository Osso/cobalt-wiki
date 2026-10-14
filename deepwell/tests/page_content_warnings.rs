//! Writing warning choices pass through live templates, includes and comment guards.
#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::page::CreatePage;
use deepwell::services::{PageService, RequestContext};
use deepwell::types::Reference;
use serde_json::json;

const SLUG: &str = "writing:warning-fixture";
const WARNINGS: [(&str, &str); 4] = [
    ("sexualContent", "sexual-content"),
    ("violence", "violence"),
    ("language", "language"),
    ("substanceAbuse", "substance-abuse"),
];
// Exact public production include: adjacent guards, not boolean conditions.
const INCLUDE: &str = "[!--{$sexualContent}--]@@@@[[image images/cw_sexual-content.png style=\"margin:0.1rem 0.5rem; alt='An icon indicating Sexual Content';\"]][!-- --][!--{$violence}--]@@@@[[image images/cw_violence.png style=\"margin:0.1rem 0.5rem; alt='An icon indicating Violence';\"]][!-- --][!--{$language}--]@@@@[[image images/cw_language.png style=\"margin:0.1rem 0.5rem; alt='An icon indicating Intense Language';\"]][!-- --][!--{$substanceAbuse}--]@@@@[[image images/cw_substance-abuse.png style=\"margin:0.1rem 0.5rem; alt='An icon indicating Substance Abuse';\"]][!-- --]";
const TEMPLATE: &str = "[[include content-warnings
| sexualContent=%%form_raw{sexualContent}%%
| violence=%%form_raw{violence}%%
| language=%%form_raw{language}%%
| substanceAbuse=%%form_raw{substanceAbuse}%%
]]

%%form_raw{content}%%

====

[[form]]
fields:
  sexualContent:
    type: select
    values:
      @@: No
      sexual-content: Yes
    default: @@
  violence:
    type: select
    values:
      @@: No
      violence: Yes
    default: @@
  language:
    type: select
    values:
      @@: No
      language: Yes
    default: @@
  substanceAbuse:
    type: select
    values:
      @@: No
      substance-abuse: Yes
    default: @@
  content:
    type: wiki
[[/form]]";
const RECORD: &str = "sexualContent: '@@'\nviolence: '@@'\nlanguage: '@@'\nsubstanceAbuse: '@@'\ncontent: 'Warning fixture body'\n";

async fn import_page(runner: &TestRunner, site_id: i64, slug: &str, source: &str) {
    PageService::import(
        runner.context(),
        CreatePage {
            site_id,
            slug: slug.into(),
            title: slug.into(),
            wikitext: source.into(),
            alt_title: None,
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Warning fixture".into(),
            tags: vec![],
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap();
}

fn assert_warnings(html: &str, selected: usize) {
    assert!(html.contains("Warning fixture body"), "{html}");
    for (index, (_, code)) in WARNINGS.iter().enumerate() {
        let image = format!("images/cw_{code}.png");
        assert_eq!(
            html.contains(&image),
            selected & (1 << index) != 0,
            "selection {selected}, {image}: {html}"
        );
    }
}

#[tokio::test]
async fn off_and_selected_warnings_agree_with_form_choices_in_view_and_preview() {
    let mut runner = TestRunner::setup_with_idle_job_workers().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    import_page(&runner, site_id, "content-warnings", INCLUDE).await;
    import_page(&runner, site_id, "writing:_template", TEMPLATE).await;
    import_page(&runner, site_id, SLUG, RECORD).await;
    let stored = run_endpoint!(runner, page_get, json!({
        "site_id": site_id, "page": SLUG, "details": {"wikitext": true, "compiled": true}
    })).unwrap();
    assert_warnings(stored.compiled_body_html.as_deref().unwrap(), 0);
    assert_eq!(stored.wikitext.as_deref(), Some(RECORD));

    runner.set_request_context(RequestContext {
        site_id: Some(site_id),
        page_reference: Some(Reference::Slug(SLUG.into())),
        user_id: Some(ADMIN_USER_ID),
        ..Default::default()
    });
    for selected in 0..16 {
        let updates: serde_json::Map<String, serde_json::Value> = WARNINGS
            .iter()
            .enumerate()
            .map(|(index, (field, code))| {
                let value = if selected & (1 << index) == 0 {
                    "@@"
                } else {
                    code
                };
                (field.to_string(), json!(value))
            })
            .collect();
        let preview = run_endpoint!(
            runner,
            page_preview,
            json!({
                "last_revision_id": stored.revision_id, "form_updates": updates
            })
        );
        assert_warnings(&preview.html, selected);
    }
    let after = run_endpoint!(runner, page_get, json!({
        "site_id": site_id, "page": SLUG, "details": {"wikitext": true, "compiled": true}
    })).unwrap();
    assert_eq!(after.revision_id, stored.revision_id);
    assert_eq!(after.wikitext, stored.wikitext);
    assert_eq!(after.compiled_body_html, stored.compiled_body_html);
}
