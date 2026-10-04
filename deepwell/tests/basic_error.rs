//! Basic error pages are HTML built from Fluent templates. Values taken from
//! the request URL (slugs, file names, block indices) must appear as text.

#[macro_use]
mod common;

use common::TestRunner;
use serde_json::json;

const PAYLOAD: &str = "\"><img src=x onerror=alert(1)>";
const ESCAPED: &str = "&quot;&gt;&lt;img src=x onerror=alert(1)&gt;";

#[tokio::test]
async fn request_values_are_escaped_in_basic_error_html() {
    let runner = TestRunner::setup().await;
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let locales = json!(["en"]);

    let outputs = [
        run_endpoint!(
            runner,
            basic_error_missing_site_slug,
            json!({"locales": locales, "site_slug": PAYLOAD}),
        ),
        run_endpoint!(
            runner,
            basic_error_missing_custom_domain,
            json!({"locales": locales, "domain": PAYLOAD}),
        ),
        run_endpoint!(
            runner,
            basic_error_missing_page_slug,
            json!({"locales": locales, "site_id": site_id, "page_slug": PAYLOAD}),
        ),
        run_endpoint!(
            runner,
            basic_error_missing_file_name,
            json!({"locales": locales, "site_id": site_id, "page_slug": "start", "filename": PAYLOAD}),
        ),
        run_endpoint!(
            runner,
            basic_error_text_block,
            json!({
                "locales": locales, "site_id": site_id, "index": PAYLOAD,
                "block_type": "html", "reason": "missing",
            }),
        ),
        run_endpoint!(
            runner,
            basic_error_blob_fetch,
            json!({"locales": locales, "s3_hash": PAYLOAD}),
        ),
    ];

    for output in outputs {
        for html in [&output.title, &output.body] {
            assert!(!html.contains("<img"), "unescaped payload: {html}");
        }
        assert!(output.body.contains(ESCAPED), "{}", output.body);
    }
}
