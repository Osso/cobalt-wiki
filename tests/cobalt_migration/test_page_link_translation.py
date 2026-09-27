import unittest

from tools.cobalt_migration.page_link_translation import translate_page_links


OLD = "https://cobalt-company.wikidot.com"
NEW = "https://cobalt-company.sakuin.org"
PAGES = {"writing:scene", "café:été", "home"}


def translate(text, pages=PAGES):
    return translate_page_links(text, OLD, NEW, pages)


class PageLinkTranslationTests(unittest.TestCase):
    def test_rewrites_destinations_only_and_preserves_labels_and_suffix_bytes(self):
        source = (
            "[http://cobalt-company.wikidot.com/writing%3Ascene?x=%2F+#frag%20x "
            "https://cobalt-company.wikidot.com/home] "
            "[[[https://cobalt-company.wikidot.com/caf%C3%A9:%C3%A9t%C3%A9?x=%25#f%2F|"
            "old https://cobalt-company.wikidot.com/home]]] "
            "[[[*https://cobalt-company.wikidot.com/home|label]]]"
        )
        result = translate(source)
        self.assertEqual(
            result.text,
            "[https://cobalt-company.sakuin.org/writing%3Ascene?x=%2F+#frag%20x "
            "https://cobalt-company.wikidot.com/home] "
            "[[[https://cobalt-company.sakuin.org/caf%C3%A9:%C3%A9t%C3%A9?x=%25#f%2F|"
            "old https://cobalt-company.wikidot.com/home]]] "
            "[[[*https://cobalt-company.sakuin.org/home|label]]]",
        )
        self.assertEqual(result.counts["rewritten"], 3)

    def test_bare_urls_remain_full_urls_with_trailing_punctuation(self):
        result = translate(
            "Visit http://cobalt-company.wikidot.com/home, then "
            "https://cobalt-company.wikidot.com/writing:scene#end."
        )
        self.assertEqual(
            result.text,
            "Visit https://cobalt-company.sakuin.org/home, then "
            "https://cobalt-company.sakuin.org/writing:scene#end.",
        )
        self.assertEqual(result.counts["rewritten"], 2)

    def test_only_exact_origin_and_confirmed_single_component_page(self):
        candidates = [
            ("/local--files/home/a.png", "non_page"),
            ("/unknown", "unconfirmed"),
            ("/home/child", "non_page"),
            ("/home%2Fchild", "non_page"),
            ("/%FF", "non_page"),
            ("/%252Fhome", "unconfirmed"),
        ]
        other = [
            "https://cobalt-company.wikidot.com.evil/home",
            "https://evilcobalt-company.wikidot.com/home",
            "https://user@cobalt-company.wikidot.com/home",
            "https://cobalt-company.wikidot.com:443/home",
            "ftp://cobalt-company.wikidot.com/home",
            "https://example.com/home",
        ]
        source = "\n".join([OLD + path for path, _ in candidates] + other)
        result = translate(source)
        self.assertEqual(result.text, source)
        for _, reason in candidates:
            self.assertIn(reason, [decision.status for decision in result.decisions])
        self.assertEqual(result.counts.get("rewritten", 0), 0)

    def test_excludes_literal_blocks_comments_raw_html_and_escaped_urls(self):
        url = OLD + "/home"
        source = "\n".join(
            [
                f'[[code type="css"]]{url}[[/code]]',
                f'[[html]]<a href="{url}">{url}</a>[[/html]]',
                f"[[module CSS]]a {{ background: url({url}) }}[[/module]]",
                f"[!-- {url} --]",
                f"@@{url}@@ @<{url}>@",
                f"<style>a {{ background: url({url}) }}</style>",
                f'<div><a href="{url}">{url}</a></div>',
                f"\\{url}",
                f"[[include {url}]]",
                f"[unrecognized {url}]",
                f"[ {url} label]",
                url,
            ]
        )
        result = translate(source)
        self.assertEqual(result.text, source[: -len(url)] + NEW + "/home")
        self.assertEqual(result.counts["rewritten"], 1)
        self.assertGreater(result.counts["unsafe_context"], 0)

    def test_explicit_mapping_uses_confirmed_target_canonical_name(self):
        result = translate_page_links(
            OLD + "/writing%3Ascene?x=%2F#frag",
            OLD,
            NEW,
            {"writing:scene": "writing:renamed"},
        )
        self.assertEqual(result.text, NEW + "/writing:renamed?x=%2F#frag")
        self.assertEqual(result.decisions[0].replacement, result.text)

    def test_escaped_markup_html_comments_and_inline_code_are_not_destinations(self):
        url = OLD + "/home"
        source = (
            f"\\[ {url} label] \\[{url} label] \\[[[{url}|label]]] "
            f"<!-- {url} --> `code {url}` {{{{ {url} }}}}"
        )
        result = translate(source)
        self.assertEqual(result.text, source)
        self.assertEqual(result.counts.get("rewritten", 0), 0)

    def test_unclosed_literal_context_does_not_rewrite_rest_of_text(self):
        source = "before [!-- " + OLD + "/home\n" + OLD + "/home"
        result = translate(source)
        self.assertEqual(result.text, source)
        self.assertEqual(result.counts.get("rewritten", 0), 0)

    def test_does_not_decode_twice_or_normalize_case_or_url_encoding(self):
        source = OLD + "/writing%3Ascene?x=%2f#Hi%20There " + OLD + "/Writing:Scene"
        result = translate(source)
        self.assertEqual(
            result.text,
            NEW + "/writing%3Ascene?x=%2f#Hi%20There " + OLD + "/Writing:Scene",
        )
        self.assertEqual(result.counts["rewritten"], 1)
        self.assertEqual(result.counts["unconfirmed"], 1)


if __name__ == "__main__":
    unittest.main()
