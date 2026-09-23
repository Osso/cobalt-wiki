//! A page's score is the sum of its votes, as Wikidot's rating.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::services::page::CreatePage;
use deepwell::services::vote::CreateVote;
use deepwell::services::{PageService, ScoreService, VoteService};
use ftml::data::ScoreValue;
use serde_json::json;

#[tokio::test]
async fn page_score_is_the_sum_of_its_votes() {
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
            slug: "score-fixture".into(),
            title: "Score fixture".into(),
            wikitext: "[[module Rate]]".into(),
            alt_title: None,
            tags: vec![],
            layout: Some(ftml::layout::Layout::Wikidot),
            revision_comments: "Score fixture".into(),
            user_id: ADMIN_USER_ID,
            bypass_filter: true,
            ip_address: common::IP_ADDRESS,
        },
    )
    .await
    .unwrap()
    .page_id;

    assert_eq!(
        ScoreService::score(ctx, page_id).await.unwrap(),
        ScoreValue::Integer(0),
        "a page without votes scores 0",
    );

    VoteService::add(
        ctx,
        CreateVote {
            page_id,
            user_id: ADMIN_USER_ID,
            value: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        ScoreService::score(ctx, page_id).await.unwrap(),
        ScoreValue::Integer(1),
    );
}
