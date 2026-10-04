//! Votes are Wikidot up/down ratings: only -1, 0 and +1 are accepted.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::prelude::*;
use deepwell::services::page::CreatePage;
use deepwell::services::{PageService, ScoreService};
use ftml::data::ScoreValue;
use serde_json::json;

#[tokio::test]
async fn vote_set_rejects_values_outside_up_down_range() {
    let runner = TestRunner::setup().await;
    let ctx = runner.context();
    let site_id = run_endpoint!(runner, site_get, json!({"site": "test"}))
        .unwrap()
        .site
        .site_id;
    let page_id = PageService::import(
        ctx,
        CreatePage {
            site_id,
            slug: "vote-range-fixture".into(),
            title: "Vote range fixture".into(),
            wikitext: "[[module Rate]]".into(),
            alt_title: None,
            tags: vec![],
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Vote range fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .page_id;

    for value in [32767, 2, -2, -32768] {
        let error = run_endpoint_err!(
            runner,
            vote_set,
            json!({"page_id": page_id, "user_id": ADMIN_USER_ID, "value": value}),
        );
        assert_contains_error!(error, ErrorType::BadRequest);
        assert_eq!(
            ScoreService::score(ctx, page_id).await.unwrap(),
            ScoreValue::Integer(0),
            "rejected vote {value} must not change the score",
        );
    }

    for (value, score) in [(1, 1), (0, 0), (-1, -1)] {
        run_endpoint!(
            runner,
            vote_set,
            json!({"page_id": page_id, "user_id": ADMIN_USER_ID, "value": value}),
        );
        assert_eq!(
            ScoreService::score(ctx, page_id).await.unwrap(),
            ScoreValue::Integer(score),
        );
    }
}
