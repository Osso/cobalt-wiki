//! Wikidot page references preserve canonical colons and explicit link labels.

use ftml::data::{PageInfo, PageRef, ScoreValue};
use ftml::layout::Layout;
use ftml::render::Render;
use ftml::render::html::HtmlRender;
use ftml::settings::{WikitextMode, WikitextSettings};
use serde_json::Value;
use std::borrow::Cow;
use std::fs;

#[test]
fn local_references_preserve_category_and_multicolon_page_names() {
    for name in [
        "home:start",
        "writing:2026-01-02:chapter-one",
        "archive:season:episode:part",
    ] {
        let reference = PageRef::parse(name).expect("valid Wikidot reference");
        assert_eq!(reference.site(), None);
        assert_eq!(reference.page(), name);
        assert_eq!(reference.extra(), None);
        assert_eq!(reference.to_string(), name);
    }
}

#[test]
fn cross_site_references_preserve_multiple_page_colons() {
    let reference = PageRef::parse(":Other-Wiki:Writing:Chapter:Part")
        .expect("valid cross-site reference");
    assert_eq!(
        reference.fields(),
        (Some("other-wiki"), "writing:chapter:part", None)
    );
    assert_eq!(reference.to_string(), ":other-wiki:writing:chapter:part");
}

#[test]
fn page_fragments_and_subpaths_remain_separate_from_canonical_identity() {
    for (input, page, extra) in [
        (
            "writing:chapter:part#section",
            "writing:chapter:part",
            "#section",
        ),
        ("writing:chapter:part/edit", "writing:chapter:part", "/edit"),
        (
            ":other-wiki:writing:chapter:part#section",
            "writing:chapter:part",
            "#section",
        ),
    ] {
        let reference = PageRef::parse(input).expect("valid reference with suffix");
        assert_eq!(reference.page(), page);
        assert_eq!(reference.extra(), Some(extra));
    }
}

#[test]
fn whitespace_and_case_normalize_without_merging_colon_segments() {
    let reference = PageRef::parse("  Writing: Chapter One : Part Two  ")
        .expect("valid mixed-case reference");
    assert_eq!(reference.page(), "writing:chapter-one:part-two");
    let cross_site = PageRef::parse(" : Other-Wiki : Writing : Part Two#Heading ")
        .expect("valid spaced cross-site reference");
    assert_eq!(
        cross_site.fields(),
        (Some("other-wiki"), "writing:part-two", Some("#Heading"))
    );
}

#[test]
fn explicit_default_category_prefix_is_removed_without_losing_remaining_colons() {
    for (input, expected) in [
        ("_default:home", "home"),
        ("_default:archive:chapter-one", "archive:chapter-one"),
    ] {
        assert_eq!(PageRef::parse(input).unwrap().page(), expected);
    }
}

#[test]
fn rendered_explicit_label_keeps_exact_multicolon_target() {
    let source = "[[[writing:chapter:part|My label]]]";
    let mut processed = source.to_owned();
    ftml::preprocess(&mut processed);
    let tokens = ftml::tokenize(&processed);
    let info = PageInfo {
        page: Cow::Borrowed("home"),
        category: None,
        site: Cow::Borrowed("cobalt-company"),
        title: Cow::Borrowed("Home"),
        alt_title: None,
        score: ScoreValue::Integer(0),
        tags: vec![],
        language: Cow::Borrowed("en"),
    };
    let settings = WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot);
    let (tree, errors) = ftml::parse(&tokens, &info, &settings).into();
    assert!(errors.is_empty(), "explicit link must parse cleanly");
    let html = HtmlRender.render(&tree, &info, &settings).body;
    assert!(html.contains("href=\"/writing:chapter:part\""), "{html}");
    assert!(html.contains(">My label</a>"), "{html}");
}

#[test]
fn protected_canonical_inventory_round_trips_when_provided() {
    let Some(path) = std::env::var_os("COBALT_CANONICAL_PLAN") else {
        return;
    };
    let plan: Value =
        serde_json::from_slice(&fs::read(path).expect("read protected import plan"))
            .expect("parse protected import plan");
    let pages = plan["pages"].as_array().expect("plan pages array");
    assert_eq!(pages.len(), 6_092);
    let multicolon = pages
        .iter()
        .filter(|entry| entry["fullname"].as_str().unwrap().matches(':').count() > 1)
        .count();
    assert_eq!(multicolon, 456);
    for entry in pages {
        let name = entry["fullname"].as_str().expect("canonical fullname");
        let reference = PageRef::parse(name).expect("canonical fullname parses");
        assert_eq!(reference.site(), None);
        assert_eq!(reference.extra(), None);
        assert_eq!(reference.page(), name, "canonical identity must be exact");
    }
}
