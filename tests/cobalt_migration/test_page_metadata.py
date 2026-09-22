"""Synthetic source-page HTML; no account data, browser state, or network calls."""

import unittest

from tools.cobalt_migration.page_metadata import (
    PageMetadataError,
    SourcePageUnavailable,
    parse_page_metadata,
)


def page_html(
    assignments=None,
    title="Sir <em>Dane</em> &amp; Atley",
    tags="<a href='/system:page-tags/tag/human#pages'>human</a><a href='/system:page-tags/tag/_completed#pages'>_completed</a><a href='/system:page-tags/tag/human#pages'>human</a>",
    info='page revision: 407, last edited: <span class="odate time_1755828499">a localized date</span>',
    content="A synthetic profile.",
):
    if assignments is None:
        assignments = (
            'WIKIREQUEST.info.pageUnixName = "character:atley";\n'
            "WIKIREQUEST.info.pageId = 1446;"
        )
    return (
        f"<html><head><script>{assignments}</script></head><body>"
        f'<div id="main-content"><h1 id="page-title">{title}</h1>'
        f'<div id="page-content">{content}</div>'
        + (
            f'<div class="page-tags"><span>{tags}</span></div>'
            if tags is not None
            else ""
        )
        + f'<div id="page-info">{info}</div></div></body></html>'
    )


class PageMetadataTests(unittest.TestCase):
    def test_complete_record_contains_only_allowlisted_metadata(self):
        html = page_html(
            assignments=(
                'WIKIREQUEST.info.pageUnixName = "character:atley";'
                "WIKIREQUEST.info.pageId = 1446;"
                'WIKIREQUEST.info.secret = "not-output";'
                'document.cookie = "not-output-either";'
            )
        )
        self.assertEqual(
            parse_page_metadata(html, expected_fullname="character:atley"),
            {
                "fullname": "character:atley",
                "page_id": 1446,
                "title": "Sir Dane & Atley",
                "tags": ["_completed", "human"],
                "revision_number": 407,
                "updated_at": 1755828499,
            },
        )

    def test_unicode_entities_nested_markup_and_unrelated_dates(self):
        html = page_html(
            title="  <span>Élan &#x2014; 雪</span> &amp; <b>amis</b>  ",
            tags="<a href='/system:page-tags/tag/%C3%A9quipe#pages'>équipe</a><a href='/system:page-tags/tag/%E9%9B%AA#pages'><b>雪</b></a><a href='/system:page-tags/tag/a%26b#pages'>a&amp;b</a>",
            content='<span class="odate time_111">not the footer</span>',
            info='page <b>revision:</b> 7, last edited: <span class="odate time_222">yesterday</span>',
        )
        record = parse_page_metadata(html)
        self.assertEqual(record["title"], "Élan — 雪 & amis")
        self.assertEqual(record["tags"], ["a&b", "équipe", "雪"])
        self.assertEqual(record["updated_at"], 222)
        self.assertEqual(record["revision_number"], 7)

    def test_tag_identity_uses_href_without_text_normalization(self):
        tags = (
            '<a href="/system:page-tags/tag/%C2%A0#pages">&nbsp;</a>'
            '<a href="/system:page-tags/tag/%C3%A9quipe#pages">other text</a>'
            '<a href="/system:page-tags/tag/_hidden#pages"></a>'
            '<a href="/system:page-tags/tag/%25C2%25A0#pages">double encoded</a>'
            '<a href="/system:page-tags/tag/e%CC%81#pages">é</a>'
            '<a href="/system:page-tags/tag/%C2%A0#pages">duplicate</a>'
        )
        self.assertEqual(
            parse_page_metadata(page_html(tags=tags))["tags"],
            sorted(["\u00a0", "équipe", "_hidden", "%C2%A0", "e\u0301"]),
        )

    def test_missing_or_malformed_tag_href_is_rejected(self):
        anchors = ["<a>human</a>", "<a href>human</a>"]
        for href in (
            "",
            "/other/tag/human#pages",
            "/system:page-tags/tag/#pages",
            "/system:page-tags/tag/human/extra#pages",
            "/system:page-tags/tag/%#pages",
            "/system:page-tags/tag/%GG#pages",
            "/system:page-tags/tag/%FF#pages",
            "/system:page-tags/tag/human?x=1#pages",
            "/system:page-tags/tag/human#other",
            "https://other.example/system:page-tags/tag/human#pages",
        ):
            anchors.append(f'<a href="{href}">human</a>')
        for anchor in anchors:
            with (
                self.subTest(anchor=anchor),
                self.assertRaisesRegex(PageMetadataError, "tag.*href"),
            ):
                parse_page_metadata(page_html(tags=anchor))

    def test_single_quoted_js_string_escapes_are_decoded_without_eval(self):
        assignments = (
            r"WIKIREQUEST.info.pageUnixName = 'character:l\'ami-\u00e9-\x61-\\';"
            "WIKIREQUEST.info.pageId = 42;"
        )
        self.assertEqual(
            parse_page_metadata(page_html(assignments=assignments))["fullname"],
            "character:l'ami-é-a-\\",
        )

    def test_double_quoted_json_string_and_surrogate_pair(self):
        assignments = (
            r'WIKIREQUEST.info.pageUnixName = "character:\ud83d\ude00";'
            "WIKIREQUEST.info.pageId = 42;"
        )
        self.assertEqual(
            parse_page_metadata(page_html(assignments=assignments))["fullname"],
            "character:😀",
        )

    def test_identical_assignments_are_consistent(self):
        assignments = (
            'WIKIREQUEST.info.pageUnixName="character:atley";'
            "WIKIREQUEST.info.pageUnixName='character:atley';"
            "WIKIREQUEST.info.pageId=42;WIKIREQUEST.info.pageId=42;"
        )
        self.assertEqual(
            parse_page_metadata(page_html(assignments=assignments))["page_id"], 42
        )

    def test_empty_tags_and_zero_revision_are_valid(self):
        record = parse_page_metadata(
            page_html(
                tags="", info='page revision: 0 <span class="odate time_0">epoch</span>'
            )
        )
        self.assertEqual(record["tags"], [])
        self.assertEqual(record["revision_number"], 0)
        self.assertEqual(record["updated_at"], 0)

    def test_absent_native_tag_container_means_no_tags(self):
        expected = parse_page_metadata(page_html(tags=""))
        self.assertEqual(
            parse_page_metadata(
                page_html(tags=None), expected_fullname="character:atley"
            ),
            expected,
        )

    def test_duplicate_tag_containers_are_ambiguous_even_when_empty(self):
        for tags in ("", "<a href='/system:page-tags/tag/human#pages'>human</a>"):
            with (
                self.subTest(tags=tags),
                self.assertRaisesRegex(PageMetadataError, "page-tags"),
            ):
                parse_page_metadata(
                    page_html(tags=tags) + '<div class="page-tags"></div>'
                )

    def test_absent_tags_do_not_bypass_required_metadata_or_denial(self):
        cases = [
            page_html(tags=None, title="Private Content"),
            page_html(tags=None, assignments="WIKIREQUEST.info.pageId=42;"),
            page_html(tags=None).replace('id="page-info"', 'id="different"'),
            page_html(tags=None, info="page revision: 2"),
        ]
        for html in cases:
            with self.subTest(html=html), self.assertRaises(PageMetadataError):
                parse_page_metadata(html)
        with self.assertRaisesRegex(PageMetadataError, "expected_fullname"):
            parse_page_metadata(page_html(tags=None), expected_fullname="other:page")

    def test_script_and_style_text_do_not_pollute_title_tags_or_footer(self):
        html = page_html(
            title="Title<script>ignored()</script><style>.unused{}</style>",
            tags="<a href='/system:page-tags/tag/human#pages'>human<script>ignored()</script></a>",
            info='page revision: 3 <script>"page revision: 999"</script><span class="odate time_4">then</span>',
        )
        record = parse_page_metadata(html)
        self.assertEqual(record["title"], "Title")
        self.assertEqual(record["tags"], ["human"])
        self.assertEqual(record["revision_number"], 3)

    def test_commented_and_string_contained_assignments_are_not_metadata(self):
        assignments = (
            "// WIKIREQUEST.info.pageId = 999;\n"
            '/* WIKIREQUEST.info.pageUnixName = "wrong"; */\n'
            'const example = "WIKIREQUEST.info.pageId = 888;";\n'
            "const example2 = `WIKIREQUEST.info.pageId = 777;`;\n"
            'WIKIREQUEST.info.pageUnixName = "character:atley";\n'
            "WIKIREQUEST.info.pageId = 42;"
        )
        self.assertEqual(
            parse_page_metadata(page_html(assignments=assignments))["page_id"], 42
        )

    def test_metadata_looking_regex_is_not_treated_as_assignments(self):
        assignments = (
            "const pattern = /WIKIREQUEST.info.pageUnixName='character:atley';"
            "WIKIREQUEST.info.pageId=42;/;"
        )
        with self.assertRaisesRegex(PageMetadataError, "missing.*pageUnixName"):
            parse_page_metadata(page_html(assignments=assignments))

    def test_native_user_agent_regex_does_not_reject_real_metadata(self):
        assignments = (
            'WIKIREQUEST.info.pageUnixName = "character:atley";\n'
            "WIKIREQUEST.info.pageId = 1446;\n"
            "var isUAMobile = !!/Android|webOS|iPhone|iPod|BlackBerry|IEMobile|Opera Mini/i.test(navigator.userAgent);"
        )
        self.assertEqual(
            parse_page_metadata(page_html(assignments=assignments)),
            parse_page_metadata(page_html()),
        )

    def test_regex_lookalikes_after_real_assignments_are_ignored(self):
        assignments = (
            'WIKIREQUEST.info.pageUnixName = "character:atley";'
            "WIKIREQUEST.info.pageId = 1446;"
            r"const pattern = /[/]WIKIREQUEST.info.pageId=999;\/"
            r"WIKIREQUEST.info.pageUnixName='wrong';[\]]/gi;"
        )
        self.assertEqual(
            parse_page_metadata(page_html(assignments=assignments)),
            parse_page_metadata(page_html()),
        )

    def test_regex_context_survives_comments_and_later_real_assignments(self):
        assignments = (
            "const pattern = !/* a comment */ /WIKIREQUEST.info.pageId=999;/;"
            'WIKIREQUEST.info.pageUnixName = "character:atley";'
            "WIKIREQUEST.info.pageId = 1446;"
        )
        self.assertEqual(
            parse_page_metadata(page_html(assignments=assignments)),
            parse_page_metadata(page_html()),
        )

    def test_malformed_regex_and_ambiguous_division_fail_explicitly(self):
        identity = (
            'WIKIREQUEST.info.pageUnixName = "character:atley";'
            "WIKIREQUEST.info.pageId = 1446;"
        )
        for expression in (
            "const pattern = /unterminated;",
            "const pattern = /[unterminated/;",
            "const pattern = /line\nbreak/;",
            "const pattern = /pattern/gg;",
            "const pattern = /pattern/z;",
            "const pattern = /pattern/v;",
            "const pattern = /trailing\\",
            "const pattern = /escaped\\\nline/;",
            "const ratio = total / count;",
            "const ratio = total++ / count;",
            "const ratio = value / WIKIREQUEST.info.pageId=999; / count;",
        ):
            with (
                self.subTest(expression=expression),
                self.assertRaises(PageMetadataError),
            ):
                parse_page_metadata(page_html(assignments=identity + expression))

    def test_assignments_in_non_javascript_script_and_article_are_not_metadata(self):
        html = page_html(assignments="")
        html = html.replace(
            "</head>",
            '<script type="application/json">{"example":"WIKIREQUEST.info.pageId = 9;"}</script></head>',
        )
        html = html.replace("A synthetic profile.", "WIKIREQUEST.info.pageId=12;")
        with self.assertRaisesRegex(PageMetadataError, "pageUnixName"):
            parse_page_metadata(html)

    def test_missing_or_empty_fields_are_rejected(self):
        cases = {
            "fullname": page_html(assignments="WIKIREQUEST.info.pageId=42;"),
            "page id": page_html(
                assignments='WIKIREQUEST.info.pageUnixName="character:atley";'
            ),
            "empty fullname": page_html(
                assignments='WIKIREQUEST.info.pageUnixName="";WIKIREQUEST.info.pageId=42;'
            ),
            "title": page_html().replace('id="page-title"', 'id="different"'),
            "empty title": page_html(title=" \n "),
            "footer": page_html().replace('id="page-info"', 'id="different"'),
            "revision": page_html(info='<span class="odate time_4">now</span>'),
            "timestamp": page_html(info="page revision: 2"),
        }
        for label, html in cases.items():
            with self.subTest(label=label), self.assertRaises(PageMetadataError):
                parse_page_metadata(html)

    def test_nonpositive_or_nonliteral_ids_are_rejected(self):
        for value in [
            "0",
            "-1",
            "1.5",
            '"42"',
            "true",
            "null",
            "0x2a",
            "42 + 1",
            "getId()",
        ]:
            with self.subTest(value=value), self.assertRaises(PageMetadataError):
                parse_page_metadata(
                    page_html(
                        assignments=f'WIKIREQUEST.info.pageUnixName="character:atley";WIKIREQUEST.info.pageId={value};'
                    )
                )

    def test_unsupported_string_expressions_and_escapes_are_rejected(self):
        for value in [
            '"character:" + "atley"',
            "getName()",
            "`character:atley`",
            r"'bad\q'",
            r"'bad\uZZZZ'",
            r'"bad\ud800"',
            "'unterminated",
        ]:
            with self.subTest(value=value), self.assertRaises(PageMetadataError):
                parse_page_metadata(
                    page_html(
                        assignments=f"WIKIREQUEST.info.pageUnixName={value};WIKIREQUEST.info.pageId=42;"
                    )
                )

    def test_conflicting_assignments_are_rejected_without_echoing_literals(self):
        for assignment in [
            "WIKIREQUEST.info.pageId=99;",
            'WIKIREQUEST.info.pageUnixName="private-other";',
        ]:
            html = page_html() + f"<script>{assignment}</script>"
            with (
                self.subTest(assignment=assignment),
                self.assertRaisesRegex(PageMetadataError, "conflicting") as caught,
            ):
                parse_page_metadata(html)
            self.assertNotIn("private-other", str(caught.exception))

    def test_expected_fullname_must_match_exactly(self):
        with self.assertRaisesRegex(PageMetadataError, "expected_fullname"):
            parse_page_metadata(page_html(), expected_fullname="character:someone-else")

    def test_conflicting_or_duplicate_html_metadata_is_rejected(self):
        cases = [
            page_html() + '<h1 id="page-title">Other title</h1>',
            page_html()
            + '<div id="page-info">page revision: 407 <span class="odate time_1755828499">date</span></div>',
            page_html(
                info='page revision: 1 page revision: 2 <span class="odate time_4">date</span>'
            ),
            page_html(
                info='page revision: 1 <span class="odate time_4">a</span><span class="odate time_5">b</span>'
            ),
        ]
        for html in cases:
            with self.subTest(html=html), self.assertRaises(PageMetadataError):
                parse_page_metadata(html)

    def test_invalid_revision_timestamp_and_blank_tag_are_rejected(self):
        cases = [
            page_html(info='page revision: -1 <span class="odate time_4">a</span>'),
            page_html(info='page revision: 1.5 <span class="odate time_4">a</span>'),
            page_html(info='page revision: 1 <span class="odate time_-4">a</span>'),
            page_html(info='page revision: 1 <span class="odate time_nope">a</span>'),
            page_html(info='page revision: 1 <span class="time_4">not an odate</span>'),
            page_html(tags="<a> </a>"),
        ]
        for html in cases:
            with self.subTest(html=html), self.assertRaises(PageMetadataError):
                parse_page_metadata(html)

    def test_void_elements_do_not_break_metadata_scope(self):
        html = page_html(
            title="Sir<br>Dane",
            tags='<a href="/system:page-tags/tag/human#pages">human<img alt="unused"></a>',
        )
        self.assertEqual(parse_page_metadata(html)["title"], "Sir Dane")

    def test_denial_and_missing_page_responses_are_distinct(self):
        for title, content, reason in [
            ("Private Content", "", "denied"),
            ("Access denied", "", "denied"),
            (
                "Player directory",
                "This area of the site is private and you don't have access to it.",
                "denied",
            ),
            ("Page not found", "", "not_found"),
            ("Missing", "The page you want to access does not exist.", "not_found"),
        ]:
            with (
                self.subTest(title=title),
                self.assertRaises(SourcePageUnavailable) as caught,
            ):
                parse_page_metadata(page_html(title=title, content=content))
            self.assertEqual(caught.exception.reason, reason)

    def test_article_mention_of_private_content_is_not_a_denial(self):
        record = parse_page_metadata(
            page_html(
                content="A guide discusses Private Content and access denied errors."
            )
        )
        self.assertEqual(record["page_id"], 1446)

    def test_error_messages_do_not_echo_unrelated_scripts_or_source_content(self):
        html = page_html(
            assignments='window.nonce="sensitive-token";', content="private prose"
        )
        with self.assertRaises(PageMetadataError) as caught:
            parse_page_metadata(html)
        self.assertNotIn("sensitive-token", str(caught.exception))
        self.assertNotIn("private prose", str(caught.exception))


if __name__ == "__main__":
    unittest.main()
