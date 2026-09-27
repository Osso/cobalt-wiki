//! Authorized paginated history listing against the integration database.

#[macro_use]
mod common;

use common::TestRunner;
use deepwell::constants::ADMIN_USER_ID;
use deepwell::error::prelude::ErrorType;
use deepwell::models::{page_revision, role_permission};
use deepwell::types::{Action, Resource};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel,
    QueryFilter,
};
use serde_json::{Value, json};

async fn fixture(runner: &TestRunner, slug: &str) -> (i64, i64, i64) {
    let site_id = run_endpoint!(runner, site_get, json!({"site":"test"}))
        .unwrap()
        .site
        .site_id;
    let created = run_endpoint!(
        runner,
        page_import,
        json!({
            "site_id": site_id, "user_id": ADMIN_USER_ID, "slug": slug,
            "title": "Current title", "wikitext": "Current body", "alt_title": null,
            "layout": "wikidot", "revision_comments": "Technical import", "bypass_filter": true,
            "ip_address": "127.0.0.1"
        })
    );
    let current = run_endpoint!(
        runner,
        page_get,
        json!({"site_id":site_id,"page":created.page_id})
    )
    .unwrap();
    (site_id, created.page_id, current.revision_id)
}

fn request(site_id: i64, page_id: i64, origin: &str) -> Value {
    json!({"site_id":site_id,"page_id":page_id,"origin":origin})
}

async fn insert_imported(
    runner: &TestRunner,
    site_id: i64,
    page_id: i64,
    current: i64,
    count: i64,
) {
    for start in (0..count).step_by(50) {
        let revisions: Vec<_> = (start..(start + 50).min(count)).map(|n| json!({
            "source_revision_id": 4_000_000 + n, "source_revision_number": n,
            "source_author_id": if n == 0 {None} else {Some(987_654)},
            "source_created_at": "2021-04-29T12:00:00Z", "source_comments": format!("Edit {n}"),
            "source_flags": if n % 2 == 0 {vec!["S", "T"]} else {vec!["F"]},
            "source_title": null,"source_slug":null,"source_tags":null,
            "wikitext":format!("Old body {n}"), "raw_source_html":"<div>Old source</div>",
            "acquired_at":"2026-09-23T00:00:00Z", "representation":"display-decoded-not-byte-exact"
        })).collect();
        run_endpoint!(
            runner,
            import_wikidot_history,
            json!({
                "site_id":site_id,"page_id":page_id,"source_page_id":1310927108,
                "expected_revision_id":current,"revisions":revisions
            })
        );
    }
}

#[tokio::test]
async fn imported_history_pages_over_two_hundred_and_filters_before_counting() {
    let runner = TestRunner::setup().await;
    let (site_id, page_id, current) = fixture(&runner, "history-list-imported").await;
    insert_imported(&runner, site_id, page_id, current, 205).await;
    let first = run_endpoint!(
        runner,
        page_history_list,
        request(site_id, page_id, "wikidot")
    );
    assert_eq!(
        (first.page, first.per_page, first.total, first.total_pages),
        (1, 20, 205, 11)
    );
    assert_eq!(
        (first.available.wikidot, first.available.local),
        (true, true)
    );
    assert_eq!(first.rows.len(), 20);
    assert_eq!((first.rows[0].id, first.rows[0].number), (4_000_204, 204));
    assert_eq!(first.rows[0].flags, vec!["S", "T"]);
    assert_eq!(first.rows[0].author_id, Some(987_654));
    assert_eq!(first.rows[0].author_name, None);
    assert!(!first.rows[0].is_current);
    assert_eq!(
        first.rows[0].representation.as_deref(),
        Some("display-decoded-not-byte-exact")
    );
    let mut second = request(site_id, page_id, "wikidot");
    second["page"] = json!(2);
    second["per_page"] = json!(200);
    let second = run_endpoint!(runner, page_history_list, second);
    assert_eq!(
        (second.total, second.total_pages, second.rows.len()),
        (205, 2, 5)
    );
    assert_eq!(second.rows[4].number, 0);
    assert_eq!(
        (
            second.rows[4].author_id,
            second.rows[4].author_name.as_deref()
        ),
        (None, None)
    );
    run_endpoint!(
        runner,
        import_wikidot_user,
        json!({
            "user_id":987_654,"created_at":"2020-11-03T08:00:00Z",
            "fetched_at":"2026-09-24T00:00:00Z","user_type":"extant",
            "name":"Archived Editor","slug":"archived-editor",
            "avatar_uploaded_blob_id":null,"real_name":null,"gender":null,
            "birthday":null,"location":null,"biography":null,"website":null,
            "karma":3,"is_pro":false,"importing_user_id":ADMIN_USER_ID,
            "ip_address":common::IP_ADDRESS
        })
    );
    let named = run_endpoint!(
        runner,
        page_history_list,
        request(site_id, page_id, "wikidot")
    );
    assert_eq!(
        named.rows[0].author_name.as_deref(),
        Some("Archived Editor")
    );
    assert_eq!(named.rows[0].author_slug, None);
    run_endpoint!(
        runner,
        user_activate_from_wikidot,
        json!({
            "user_id": 987_654, "user_type": "regular",
            "email": "wikidot-987654@members.invalid", "locales": ["en"],
            "password": "secret nobody is told",
            "bypass_filter": true, "bypass_email_verification": true,
            "ip_address": common::IP_ADDRESS,
        })
    );
    let member = run_endpoint!(
        runner,
        page_history_list,
        request(site_id, page_id, "wikidot")
    );
    assert_eq!(
        member.rows[0].author_slug.as_deref(),
        Some("archived-editor")
    );
    let mut filtered = request(site_id, page_id, "wikidot");
    filtered["per_page"] = json!(10);
    filtered["filters"] = json!({"all":false,"title":true,"files":true});
    let filtered = run_endpoint!(runner, page_history_list, filtered);
    assert_eq!((filtered.total, filtered.total_pages), (205, 21));
    let mut just_files = request(site_id, page_id, "wikidot");
    just_files["filters"] = json!({"all":false,"files":true});
    let just_files = run_endpoint!(runner, page_history_list, just_files);
    assert_eq!((just_files.total, just_files.total_pages), (102, 6));
    assert_eq!(just_files.rows[0].number, 203);
    let mut empty = request(site_id, page_id, "wikidot");
    empty["filters"] = json!({"all":false});
    let empty = run_endpoint!(runner, page_history_list, empty);
    assert_eq!(empty.total, 205);
    let source = run_endpoint!(
        runner,
        page_imported_revision,
        json!({"site_id":site_id,"page_id":page_id,"source_revision_number":0})
    )
    .unwrap();
    assert_eq!(source.wikitext, "Old body 0");
    let current_after =
        run_endpoint!(runner, page_get, json!({"site_id":site_id,"page":page_id}))
            .unwrap();
    assert_eq!(current_after.revision_id, current);
}

#[tokio::test]
async fn native_history_uses_actual_revision_changes_and_local_name() {
    let runner = TestRunner::setup().await;
    let (site_id, page_id, current) = fixture(&runner, "history-list-native").await;
    let original = page_revision::Entity::find_by_id(current)
        .one(runner.context().transaction())
        .await
        .unwrap()
        .unwrap();
    for number in 1..=21 {
        let mut revision = original.clone().into_active_model();
        revision.revision_id = sea_orm::ActiveValue::NotSet;
        revision.revision_number = Set(number);
        revision.changes = Set(match number % 3 {
            0 => vec!["wikitext".into(), "title".into()],
            1 => vec!["slug".into()],
            _ => vec!["tags".into()],
        });
        revision.revision_type = Set(if number % 3 == 1 {
            deepwell::types::PageRevisionType::Move
        } else {
            deepwell::types::PageRevisionType::Regular
        });
        revision.comments = Set(format!("Edit {number}"));
        if number == 21 {
            revision.hidden = Set(vec!["comments".into(), "title".into()]);
        }
        revision
            .insert(runner.context().transaction())
            .await
            .unwrap();
    }
    let first = run_endpoint!(
        runner,
        page_history_list,
        request(site_id, page_id, "local")
    );
    assert_eq!(
        (first.total, first.total_pages, first.rows.len()),
        (22, 2, 20)
    );
    assert_eq!(first.rows[0].number, 21);
    assert_eq!(first.rows[0].flags, vec!["S", "T"]);
    assert_eq!(first.rows[0].comments, "");
    assert_eq!(first.rows[0].representation, None);
    let mut last = request(site_id, page_id, "local");
    last["page"] = json!(2);
    let last = run_endpoint!(runner, page_history_list, last);
    assert_eq!(last.rows[1].number, 0);
    assert_eq!(last.rows[1].flags, vec!["N", "S", "T", "A", "M"]);
    assert!(last.rows[1].is_current);
    assert_eq!(last.rows[1].id, current);
    assert_eq!(last.rows[1].author_id, Some(ADMIN_USER_ID));
    assert!(last.rows[1].author_name.is_some());
    assert_eq!(last.rows[1].author_slug.as_deref(), Some("administrator"));
    let mut title = request(site_id, page_id, "local");
    title["filters"] = json!({"all":false,"title":true});
    let title = run_endpoint!(runner, page_history_list, title);
    assert_eq!(title.total, 8);
    assert!(title.rows.iter().all(|row| row.flags.contains(&"T".into())));
    let mut files = request(site_id, page_id, "local");
    files["filters"] = json!({"all":false,"files":true});
    let files = run_endpoint!(runner, page_history_list, files);
    assert_eq!((files.total, files.rows.len()), (0, 0));
}

#[tokio::test]
async fn history_tags_and_metadata_are_distinct_and_combine_with_or() {
    let runner = TestRunner::setup().await;
    let (site_id, page_id, current) = fixture(&runner, "history-list-tags-filter").await;
    let revisions: Vec<_> = [(1, "A"), (2, "M"), (3, "S")]
        .into_iter()
        .map(|(number, flag)| {
            json!({
                "source_revision_id": 5_000_000 + number,
                "source_revision_number": number,
                "source_author_id": null,
                "source_created_at": "2021-04-29T12:00:00Z",
                "source_comments": "",
                "source_flags": [flag],
                "source_title": null, "source_slug": null, "source_tags": null,
                "wikitext": "Old body", "raw_source_html": "<div>Old source</div>",
                "acquired_at": "2026-09-23T00:00:00Z",
                "representation": "display-decoded-not-byte-exact"
            })
        })
        .collect();
    run_endpoint!(
        runner,
        import_wikidot_history,
        json!({
            "site_id": site_id, "page_id": page_id, "source_page_id": 1310927108,
            "expected_revision_id": current, "revisions": revisions
        })
    );

    let original = page_revision::Entity::find_by_id(current)
        .one(runner.context().transaction())
        .await
        .unwrap()
        .unwrap();
    for (number, change) in [(1, "tags"), (2, "alt_title"), (3, "wikitext")] {
        let mut revision = original.clone().into_active_model();
        revision.revision_id = sea_orm::ActiveValue::NotSet;
        revision.revision_number = Set(number);
        revision.revision_type = Set(deepwell::types::PageRevisionType::Regular);
        revision.changes = Set(vec![change.into()]);
        revision
            .insert(runner.context().transaction())
            .await
            .unwrap();
    }

    for origin in ["wikidot", "local"] {
        let baseline_tags = if origin == "local" {
            vec![1, 0]
        } else {
            vec![1]
        };
        let baseline_meta = if origin == "local" {
            vec![2, 0]
        } else {
            vec![2]
        };
        let combined = if origin == "local" {
            vec![2, 1, 0]
        } else {
            vec![2, 1]
        };
        for (filters, expected) in [
            (json!({"tags": true}), baseline_tags),
            (json!({"meta": true}), baseline_meta),
            (json!({"tags": true, "meta": true}), combined),
        ] {
            let mut input = request(site_id, page_id, origin);
            input["filters"] = filters;
            let listed = run_endpoint!(runner, page_history_list, input);
            let numbers: Vec<_> = listed.rows.iter().map(|row| row.number).collect();
            let mut expected = expected;
            expected.sort_unstable_by(|a, b| b.cmp(a));
            assert_eq!(numbers, expected, "origin={origin}");
            assert_eq!(listed.total, numbers.len() as u64);
        }
    }
    let native = run_endpoint!(
        runner,
        page_history_list,
        request(site_id, page_id, "local")
    );
    assert_eq!(native.rows[0].flags, vec!["S"]);
    assert_eq!(native.rows[1].flags, vec!["M"]);
    assert_eq!(native.rows[2].flags, vec!["A"]);
}

#[tokio::test]
async fn history_listing_validates_inputs_and_checks_page_view_before_data() {
    let runner = TestRunner::setup().await;
    let (site_id, page_id, current) =
        fixture(&runner, "history-list-authorization").await;
    insert_imported(&runner, site_id, page_id, current, 1).await;
    for (key, value) in [
        ("page", json!(0)),
        ("page", json!(-1)),
        ("per_page", json!(25)),
        ("per_page", json!(0)),
        ("filters", json!({"unexpected":true})),
    ] {
        let mut invalid = request(site_id, page_id, "wikidot");
        invalid[key] = value;
        let error = run_endpoint_err!(runner, page_history_list, invalid);
        assert_contains_error!(error, ErrorType::DatabaseImport);
    }
    for origin in ["wikidot", "local"] {
        let error =
            run_endpoint_err!(runner, page_history_list, request(-1, page_id, origin));
        assert_contains_error!(error, ErrorType::PageNotFound);
    }
    role_permission::Entity::delete_many()
        .filter(role_permission::Column::SiteId.eq(site_id))
        .filter(role_permission::Column::ResourceType.eq(Resource::Page))
        .filter(role_permission::Column::Action.eq(Action::View))
        .exec(runner.context().transaction())
        .await
        .unwrap();
    for origin in ["wikidot", "local"] {
        let error = run_endpoint_err!(
            runner,
            page_history_list,
            request(site_id, page_id, origin)
        );
        assert_contains_error!(error, ErrorType::Permission);
    }
}
