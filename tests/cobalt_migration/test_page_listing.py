"""Synthetic listing fixtures only; no browser or private source data."""

import unittest

from tools.cobalt_migration.page_listing import PageListingError, parse_page_listing
from tools.cobalt_migration.page_metadata import SourcePageUnavailable

ORIGIN = "https://wiki.example"


def listing(*blocks):
    return (
        '<div id="page-content">'
        + "".join(f'<div class="list-pages-box">{block}</div>' for block in blocks)
        + "</div>"
    )


class PageListingTests(unittest.TestCase):
    def test_multiple_blocks_preserve_order_and_deduplicate(self):
        html = listing(
            '<a href="/character:one">One</a><a href="/_hidden_name">Hidden</a>',
            '<a href="/character:one">Again</a><a href="/writing:two">Two</a>',
        )
        self.assertEqual(
            parse_page_listing(html, ORIGIN),
            {
                "fullnames": ["character:one", "_hidden_name", "writing:two"],
                "highest_page": 1,
            },
        )

    def test_shared_pagination_and_clamped_repeated_blocks(self):
        first = listing(
            '<a href="/writing:one">One</a><div class="pager"><span class="pager-no">page 1 of 3</span><a href="/pagelist/p/3">last</a></div>',
            '<a href="/home:start">Start</a>',
        )
        second = listing(
            '<a href="/writing:two">Two</a><div class="pager"><span class="pager-no">page 2 of 3</span></div>',
            '<a href="/home:start">Start</a>',
        )
        self.assertEqual(
            parse_page_listing(first, ORIGIN),
            {"fullnames": ["writing:one", "home:start"], "highest_page": 3},
        )
        self.assertEqual(
            parse_page_listing(second, ORIGIN),
            {"fullnames": ["writing:two", "home:start"], "highest_page": 3},
        )

    def test_url_forms_and_encoding_preserve_literal_names(self):
        html = listing(
            "".join(
                f'<a href="{url}">Page</a>'
                for url in (
                    "/character:under_score",
                    "writing%3A%C3%A9lan",
                    "https://WIKI.example:443/_secret",
                    "//wiki.example/arc:one",
                    "/writing%253Atwo",
                    "/character:under_score#section",
                )
            )
        )
        self.assertEqual(
            parse_page_listing(html, ORIGIN)["fullnames"],
            [
                "character:under_score",
                "writing:élan",
                "_secret",
                "arc:one",
                "writing%3Atwo",
            ],
        )

    def test_excludes_nonpage_links_and_inert_markup(self):
        html = listing("""<a href="/good">Good</a>
            <a href="https://elsewhere.example/bad">External</a>
            <a href="http://wiki.example/bad">Wrong scheme</a>
            <a href="https://wiki.example:444/bad">Wrong port</a>
            <a href="#section">Fragment</a><a href="?sort=name">Query</a>
            <a href="/good/edit/true"><b>Edit</b></a>
            <a href="/other"> Edit </a>
            <div class="pager"><a href="/pagelist/p/7">Last</a><a href="/noise">Noise</a></div>
            <template><a href="/fake">Fake</a></template>
            <script>"<a href='/fake2'>Fake</a>"</script>
            <a href="mailto:someone@example.com">Mail</a>
            <a href="/nested/path">Route</a><a href="/">Home route</a>""")
        self.assertEqual(
            parse_page_listing(html, ORIGIN), {"fullnames": ["good"], "highest_page": 7}
        )

    def test_ignores_links_and_pagers_outside_listing(self):
        html = (
            '<a href="/outside">Outside</a><span class="pager-no">999</span>'
            + listing('<a href="/inside">Inside</a>')
        )
        self.assertEqual(
            parse_page_listing(html, ORIGIN),
            {"fullnames": ["inside"], "highest_page": 1},
        )

    def test_empty_list_is_valid_but_missing_structure_is_not(self):
        self.assertEqual(
            parse_page_listing(listing(""), ORIGIN),
            {"fullnames": [], "highest_page": 1},
        )
        for html in (
            "",
            '<div id="page-content"></div>',
            '<div class="list-pages-box"></div>',
        ):
            with self.subTest(html=html), self.assertRaises(PageListingError):
                parse_page_listing(html, ORIGIN)

    def test_denial_and_not_found_are_distinguished(self):
        for html, reason in (
            ('<h1 id="page-title">Private content</h1>', "denied"),
            (
                '<div id="page-content">This area of the site is private.</div>',
                "denied",
            ),
            ('<h1 id="page-title">Page not found</h1>', "not_found"),
        ):
            with (
                self.subTest(reason=reason),
                self.assertRaises(SourcePageUnavailable) as caught,
            ):
                parse_page_listing(html, ORIGIN)
            self.assertEqual(caught.exception.reason, reason)

    def test_ambiguous_or_malformed_encoding_fails_explicitly(self):
        for path in (
            "/bad%ZZ",
            "/bad%FF",
            "/name%2Fpart",
            "/bad%00name",
            "/bad%20name",
        ):
            with self.subTest(path=path), self.assertRaises(PageListingError):
                parse_page_listing(listing(f'<a href="{path}">Page</a>'), ORIGIN)

    def test_invalid_source_origin_fails(self):
        for origin in (
            "wiki.example",
            "ftp://wiki.example",
            "https://wiki.example/path",
            "https://user:pass@wiki.example",
        ):
            with self.subTest(origin=origin), self.assertRaises(PageListingError):
                parse_page_listing(listing(""), origin)


if __name__ == "__main__":
    unittest.main()
