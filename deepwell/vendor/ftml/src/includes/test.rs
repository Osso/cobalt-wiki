/*
 * includes/test.rs
 *
 * ftml - Library to parse Wikidot text
 * Copyright (C) 2019-2026 Wikijump Team
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */

use super::{
    DebugIncluder, FetchedPage, IncludeRef, Includer, PageRef, include, parse_includes,
};
use crate::layout::Layout;
use crate::settings::{WikitextMode, WikitextSettings};
use std::borrow::Cow;
use std::convert::Infallible;

struct CardIncluder;

impl<'t> Includer<'t> for CardIncluder {
    type Error = Infallible;

    fn include_pages(
        &mut self,
        includes: &[IncludeRef<'t>],
    ) -> Result<Vec<FetchedPage<'t>>, Infallible> {
        Ok(includes
            .iter()
            .map(|include| FetchedPage {
                page_ref: include.page_ref().clone(),
                content: Some(Cow::Borrowed("{$leading}|{$empty}|{$trailing}")),
            })
            .collect())
    }

    fn no_such_include(&mut self, _: &PageRef) -> Result<Cow<'t, str>, Infallible> {
        unreachable!("test card always exists")
    }
}

#[test]
fn empty_middle_argument_substitutes_empty_without_losing_adjacent_values() {
    let input = "[[include card | leading=Left| empty= | trailing=Right]]";
    let directives = parse_includes(input);
    assert_eq!(directives.len(), 1);
    assert_eq!(
        directives[0].1.variables().get("empty").map(AsRef::as_ref),
        Some("")
    );

    let settings = WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot);
    let (output, pages) = include(input, &settings, CardIncluder, || panic!()).unwrap();
    assert_eq!(output, "Left||Right");
    assert_eq!(pages, vec![PageRef::page_only("card")]);
}

#[test]
fn empty_final_argument_substitutes_empty_and_keeps_first_value() {
    let input = "[[include card | leading=Left| trailing=Right| empty=]]";
    let directives = parse_includes(input);
    assert_eq!(directives.len(), 1);
    assert_eq!(
        directives[0].1.variables().get("empty").map(AsRef::as_ref),
        Some("")
    );

    let settings = WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot);
    let (output, pages) = include(input, &settings, CardIncluder, || panic!()).unwrap();
    assert_eq!(output, "Left||Right");
    assert_eq!(pages, vec![PageRef::page_only("card")]);

    let nav_input = "[[include card | user2=]]";
    let nav_directives = parse_includes(nav_input);
    assert_eq!(nav_directives.len(), 1);
    assert_eq!(
        nav_directives[0]
            .1
            .variables()
            .get("user2")
            .map(AsRef::as_ref),
        Some("")
    );

    let first_empty =
        "[[include card | leading=Left| empty= | empty=Fallback| trailing=Right]]";
    let (output, pages) =
        include(first_empty, &settings, CardIncluder, || panic!()).unwrap();
    assert_eq!(output, "Left||Right");
    assert_eq!(pages, vec![PageRef::page_only("card")]);
}

#[test]
fn argument_values_exclude_whitespace_before_the_next_separator() {
    let input = "[[include card\n| leading=Left \n| empty= \t\n| trailing=Right\n]]";
    let settings = WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot);
    let (output, _) = include(input, &settings, CardIncluder, || panic!()).unwrap();
    assert_eq!(output, "Left||Right");
}

#[test]
fn segments_without_equals_are_ignored_like_wikidot() {
    // arc:season-5: an instruction segment sits between real arguments.
    let input = "[[include card\n| leading=Left\n| Place a hyphen in either IC or OOC (not both) to indicate the status of the event: | empty= \n| trailing=Right\n]]";
    let directives = parse_includes(input);
    assert_eq!(directives.len(), 1);
    assert_eq!(directives[0].0, 0..input.len());
    assert_eq!(directives[0].1.variables().len(), 3);

    let settings = WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot);
    let (output, _) = include(input, &settings, CardIncluder, || panic!()).unwrap();
    assert_eq!(output, "Left||Right");
}

#[test]
fn scans_directive_ranges_references_and_arguments() {
    let input =
        "Before\n[[include component:card | label=Hello]]\n[[include :other:theme:test]]";
    let directives = parse_includes(input);

    assert_eq!(directives.len(), 2);
    assert_eq!(
        &input[directives[0].0.clone()],
        "[[include component:card | label=Hello]]"
    );
    assert_eq!(
        directives[0].1.page_ref(),
        &PageRef::page_only("component:card")
    );
    assert_eq!(
        directives[0].1.variables().get("label").map(AsRef::as_ref),
        Some("Hello")
    );
    assert_eq!(
        &input[directives[1].0.clone()],
        "[[include :other:theme:test]]"
    );
    assert_eq!(
        directives[1].1.page_ref(),
        &PageRef::page_and_site("other", "theme:test")
    );
    assert!(parse_includes("Nothing to include").is_empty());
    assert!(parse_includes("[[include ]]").is_empty());
}

#[test]
fn extraction_keeps_existing_include_output_and_disabled_syntax() {
    let input = "[[include apple name=Pear]]\n[[include banana]]";
    let settings = WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot);
    let (output, pages) = include(input, &settings, DebugIncluder, || panic!()).unwrap();
    assert_eq!(output, "<MISSING-PAGE apple>\n<INCLUDED-PAGE banana {}>");
    assert_eq!(
        pages,
        vec![PageRef::page_only("apple"), PageRef::page_only("banana")]
    );

    let disabled = WikitextSettings::from_mode(WikitextMode::ForumPost, Layout::Wikidot);
    let (output, pages) = include(input, &disabled, DebugIncluder, || panic!()).unwrap();
    assert_eq!(output, input);
    assert!(pages.is_empty());
}

#[test]
fn includes() {
    let settings = WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot);

    macro_rules! test {
        ($text:expr, $expected:expr $(,)?) => {{
            let mut text = str!($text);
            let result = include(&mut text, &settings, DebugIncluder, || panic!());
            let (output, actual) = result.expect("Fetching pages failed");
            let expected = $expected;

            println!("Input:  '{}'", $text);
            println!("Output: '{}'", &output);
            println!("Pages (actual):   {:?}", &actual);
            println!("Pages (expected): {:?}", &expected);
            println!();

            assert_eq!(
                &actual, &expected,
                "Actual pages to include doesn't match expected"
            );
        }};
    }

    // Valid cases

    test!("", vec![]);
    test!("[[include page]]", vec![PageRef::page_only("page")]);
    test!("[[include page ]]", vec![PageRef::page_only("page")]);
    test!("[[include page ]]", vec![PageRef::page_only("page")]);
    test!("[[ include page ]]", vec![PageRef::page_only("page")]);
    test!("[[include page |]]", vec![PageRef::page_only("page")]);
    test!("[[include page | ]]", vec![PageRef::page_only("page")]);
    test!("[[include page ||]]", vec![PageRef::page_only("page")]);
    test!("[[include page || ]]", vec![PageRef::page_only("page")]);

    test!("[[include PAGE]]", vec![PageRef::page_only("PAGE")]);
    test!("[[include PAGE ]]", vec![PageRef::page_only("PAGE")]);
    test!("[[include PAGE ]]", vec![PageRef::page_only("PAGE")]);
    test!("[[ include PAGE ]]", vec![PageRef::page_only("PAGE")]);

    // Arguments
    test!("[[include apple a =1]]", vec![PageRef::page_only("apple")]);
    test!("[[include apple a= 1]]", vec![PageRef::page_only("apple")]);
    test!("[[include apple a = 1]]", vec![PageRef::page_only("apple")]);
    test!(
        "[[include apple a = 1 ]]",
        vec![PageRef::page_only("apple")],
    );
    test!(
        "[[include apple  a = 1 ]]",
        vec![PageRef::page_only("apple")],
    );

    test!("[[include banana a=1]]", vec![PageRef::page_only("banana")]);
    test!(
        "[[include banana a=1|]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana a=1||]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana a=1| |]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana a=1|||]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana a=1| |  |]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana a=1 |]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana a=1 ||]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana a=1 | |]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana |a=1]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana ||a=1]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana | |a=1]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana | a=1]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana || a=1]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana |a=1|]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana ||a=1|]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana | a=1|]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana || a=1|]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana |a=1 |]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana |a=1 ||]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana ||a=1 |]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana ||a=1 ||]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana | a=1 |]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana | a=1 ||]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana || a=1 |]]",
        vec![PageRef::page_only("banana")],
    );
    test!(
        "[[include banana || a=1 ||]]",
        vec![PageRef::page_only("banana")],
    );

    test!(
        "[[include cherry a=1|b=2]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry a=1||b=2]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry a=1|b=2|]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry a=1|b=2||]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry a=1||b=2|]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry a=1||b=2||]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry a=1 |b=2 |]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry a=1 |b=2 ||]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry a=1 ||b=2 |]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry |a=1|b=2]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry |a=1||b=2]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry ||a=1|b=2]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry | a=1| b=2]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry | a=1|| b=2]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry || a=1| b=2]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry |a=1|b=2|]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry |a=1|b=2|]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry |a=1|b=2||]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry |a=1||b=2|]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry ||a=1|b=2|]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry ||a=1||b=2||]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry | a=1| b=2|]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry |a=1 |b=2 |]]",
        vec![PageRef::page_only("cherry")],
    );
    test!(
        "[[include cherry | a=1 | b=2 |]]",
        vec![PageRef::page_only("cherry")],
    );

    test!(
        "[[include durian a=1|b=2|C=**]]",
        vec![PageRef::page_only("durian")],
    );
    test!(
        "[[include durian a=1|b=2|C=**|]]",
        vec![PageRef::page_only("durian")],
    );
    test!(
        "[[include durian a=1 |b=2 |C=** |]]",
        vec![PageRef::page_only("durian")],
    );
    test!(
        "[[include durian |a=1|b=2|C=**]]",
        vec![PageRef::page_only("durian")],
    );
    test!(
        "[[include durian | a=1| b=2| C=**]]",
        vec![PageRef::page_only("durian")],
    );
    test!(
        "[[include durian |a=1|b=2|C=**|]]",
        vec![PageRef::page_only("durian")],
    );
    test!(
        "[[include durian | a=1| b=2| C=**|]]",
        vec![PageRef::page_only("durian")],
    );
    test!(
        "[[include durian |a=1 |b=2 |C=** |]]",
        vec![PageRef::page_only("durian")],
    );
    test!(
        "[[include durian | a=1 | b=2 | C=** ]]",
        vec![PageRef::page_only("durian")],
    );

    // Off-site includes
    test!(
        "[[include component:my-thing]]",
        vec![PageRef::page_only("component:my-thing")],
    );
    test!(
        "[[include :scp-wiki:main]]",
        vec![PageRef::page_and_site("scp-wiki", "main")],
    );
    test!(
        "[[include :scp-wiki:component:my-thing]]",
        vec![PageRef::page_and_site("scp-wiki", "component:my-thing")],
    );
    test!(
        "[[include :scp-wiki:deleted:protected:component:magic]]",
        vec![PageRef::page_and_site(
            "scp-wiki",
            "deleted:protected:component:magic"
        )],
    );

    // Multiple includes
    test!(
        "A\n[[include B]]\nC\n[[include D]]\nE\n[[include F]]\nG",
        vec![
            PageRef::page_only("B"),
            PageRef::page_only("D"),
            PageRef::page_only("F"),
        ],
    );
    test!(
        "[[include my-page]]\n[[include :scp-wiki:theme:black-highlighter-theme]]\n",
        vec![
            PageRef::page_only("my-page"),
            PageRef::page_and_site("scp-wiki", "theme:black-highlighter-theme"),
        ],
    );

    // Multi-line includes
    test!("[[include page\n]]", vec![PageRef::page_only("page")]);
    test!(
        "[[include component:multi-line | contents= \nSome content here \nMore stuff]]",
        vec![PageRef::page_only("component:multi-line")],
    );
    test!(
        "[[include component:multi-line argument=x | contents= \nSome content here \nMore stuff \n|]]",
        vec![PageRef::page_only("component:multi-line")],
    );
    test!(
        "[[include component:multi-line | contents= \nSome content here\nMore stuff\n]]",
        vec![PageRef::page_only("component:multi-line")],
    );
    test!(
        "[[include component:multi-line | contents=\nSome content here\nMore stuff\n]]",
        vec![PageRef::page_only("component:multi-line")],
    );
    test!(
        "My wonderful page!\n\n[[include component:info-ayers\n\tlang=en |\n\tpage=scp-xxxx |\n\tauthorPage=http://scpwiki.com/main |\n\tcomments=\n**SCP-XXXX:** My amazing skip \n**Author:** [[*user Username]] \n]]",
        vec![PageRef::page_only("component:info-ayers")],
    );
    test!(
        "My other wonderful page!\n\n[[include component:info-ayers\n\t|lang=en\n\t|page=scp-xxxx\n\t|authorPage=http://scpwiki.com/main\n\t|comments=\n**SCP-XXXX:** My amazing skip \n**Author:** [[*user Username]] \n]]",
        vec![PageRef::page_only("component:info-ayers")],
    );

    // Invalid cases

    test!("other text", vec![]);
    test!("include]]", vec![]);
    test!("[[include", vec![]);
    test!("[[include]]", vec![]);
    test!("[[include ]]", vec![]);
    test!("[[ include]]", vec![]);

    test!(
        "[[include component:multi-line | contents= \nSome content here \nMore stuff",
        vec![],
    );
}
