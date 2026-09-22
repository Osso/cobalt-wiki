import unittest

from tools.cobalt_migration.poc_theme import render_theme


class ThemeTests(unittest.TestCase):
    def test_embeds_font_and_rewrites_only_source_attachment_urls(self):
        theme = """[[code type="css"]]
@import url(/admin:font/code/1);
#header { background: url(http://cobalt-company.wdfiles.com/local--files/images/logo.png); }
body { background: black; }
[[/code]]"""
        font = """[[code type="css"]]
@font-face { font-family: Cobalt; src: url(data:font/woff;base64,YWJj); }
[[/code]]"""
        result = render_theme(theme, font)
        self.assertIn("url(data:font/woff;base64,YWJj)", result)
        self.assertIn("url(/-/file/images/logo.png)", result)
        self.assertIn("body { background: black; }", result)
        self.assertNotIn("/admin:font/code/1", result)
        self.assertNotIn("[[code", result)
        self.assertLess(result.index("@font-face"), result.index("#header"))

    def test_rejects_missing_or_ambiguous_css_blocks(self):
        block = '[[code type="css"]]body {}[[/code]]'
        for value in ["body {}", block + block]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                render_theme(value, block)

    def test_rejects_unexpected_font_import_contract(self):
        with self.assertRaises(ValueError):
            render_theme(
                '[[code type="css"]]body {}[[/code]]',
                '[[code type="css"]]@font-face {}[[/code]]',
            )


if __name__ == "__main__":
    unittest.main()
