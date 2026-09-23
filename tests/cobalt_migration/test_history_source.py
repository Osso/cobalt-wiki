"""Wikidot history display decoding, not byte-exact historical recovery."""

import unittest

from tools.cobalt_migration.history_source import (
    HistorySourceError,
    decode_history_source,
)


class HistorySourceTest(unittest.TestCase):
    def decode(self, contents):
        return decode_history_source(
            f'<section>UI</section><div class="page-source">\n{contents}\n</div>'
        )

    def test_empty_body_and_single_newline_keep_source_whitespace(self):
        self.assertEqual(self.decode("")["wikitext"], "")
        self.assertEqual(self.decode("<br />\n")["wikitext"], "\n")
        self.assertEqual(self.decode("\n<br />\n\n")["wikitext"], "\n\n\n")
        self.assertEqual(
            self.decode("&nbsp;leading<br />\ntrailing&nbsp;")["wikitext"],
            " leading\ntrailing ",
        )

    def test_decodes_html_once_and_preserves_literal_nonbreaking_spaces(self):
        result = self.decode(
            "&lt;x&gt; &amp;nbsp; &amp;amp; &quot;q&quot; &#160; \u00a0"
        )
        self.assertEqual(result["wikitext"], '<x> &nbsp; &amp; "q" \u00a0 \u00a0')
        self.assertEqual(result["representation"], "display-decoded-not-byte-exact")

    def test_semipre_spaces_and_tab_are_display_text_not_original_bytes(self):
        self.assertEqual(
            self.decode("&nbsp;&nbsp;&nbsp;X<br />\n&nbsp;&nbsp;Y")["wikitext"],
            "   X\n  Y",
        )
        self.assertEqual(self.decode("&nbsp;&nbsp;&nbsp;&nbsp;X")["wikitext"], "    X")

    def test_rejects_missing_multiple_and_ambiguous_source_containers(self):
        for html in (
            "<div>not source</div>",
            '<div class="page-source">\nx\n</div><div class="page-source">\ny\n</div>',
            '<div class="page-source">x</div>',
            '<div class="page-source">\nx',
            '<div class="page-source">\nx\n<b>unexpected</b></div>',
            '<div class="page-source">\nx<br />y\n</div>',
            '<div class="page-source">\nx<br />\r\ny\n</div>',
        ):
            with self.subTest(html=html[:32]), self.assertRaises(HistorySourceError):
                decode_history_source(html)

    def test_rejects_unknown_entities_and_malformed_responses(self):
        for html in (
            '<div class="page-source">\n&unknown;\n</div>',
            '<div class="page-source">\n&#x110000;\n</div>',
            '<div class="page-source">\n&broken\n</div>',
            '<div class="page-source">\nhello\n</div><',
        ):
            with self.subTest(html=html[:35]), self.assertRaises(HistorySourceError):
                decode_history_source(html)


if __name__ == "__main__":
    unittest.main()
