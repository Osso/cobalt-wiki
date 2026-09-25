"""Pure source-reference payload contract; no API or source archive access."""

import unittest

from tools.cobalt_migration.worldanvil_reference import reference_payload


SOURCE = {
    "fullname": "template:infobox",
    "title": "Infobox",
    "tags": ["reference", "wikidot"],
}


class ReferencePayloadTests(unittest.TestCase):
    def test_identity_privacy_and_source_reference_label(self):
        payload = reference_payload(SOURCE, "key: value")
        self.assertEqual(payload["templateType"], "article")
        self.assertEqual(payload["state"], "private")
        self.assertEqual(payload["editor"], "plutarch")
        self.assertIn("Source reference", payload["title"])
        self.assertEqual(payload["title"], "Source reference: template:infobox")
        self.assertIn("Infobox", payload["content"])
        self.assertIn("template:infobox", payload["content"])
        self.assertEqual(
            payload["tags"].split(","),
            [
                "reference",
                "wikidot",
                "source-reference",
                "cobalt-source:template:infobox",
            ],
        )

    def test_preserves_lines_and_wikidot_markup_without_executing_it(self):
        raw = (
            "---\n"
            "field: [include template:infobox]\n"
            "[code]x[/code]\n"
            "\n"
            "[[module CSS]]\n"
            "<style>body {color:red}</style>\n"
            '<widget attr="x&y">\n'
        )
        content = reference_payload(SOURCE, raw)["content"]
        source_section = content.split("Source text:[br]", 1)[1]
        self.assertEqual(
            source_section,
            "[code][noparse]---[/noparse][br]"
            "[noparse]field: [include template:infobox][/noparse][br]"
            "[noparse][code]x[/code][/noparse][br]"
            "[br]"
            "[noparse][[module CSS]][/noparse][br]"
            "[noparse]&lt;style&gt;body {color:red}&lt;/style&gt;[/noparse][br]"
            "[noparse]&lt;widget attr=&quot;x&amp;y&quot;&gt;[/noparse][br][/code]",
        )

    def test_reference_identity_does_not_require_unavailable_metadata_title(self):
        source = {"fullname": "player:_public", "tags": [], "status": "denied"}
        payload = reference_payload(source, "Visible source from the supplied backup")
        self.assertEqual(payload["title"], "Source reference: player:_public")
        self.assertIn(
            "Original title unavailable in metadata export", payload["content"]
        )
        self.assertIn("Visible source from the supplied backup", payload["content"])

    def test_noparse_closer_in_source_is_rejected(self):
        for raw in ("before [/noparse] after", "before [/NoParse] after"):
            with (
                self.subTest(raw=raw),
                self.assertRaisesRegex(ValueError, "noparse closing tag"),
            ):
                reference_payload(SOURCE, raw)

    def test_source_identity_cannot_close_literal_wrapper(self):
        source = {**SOURCE, "title": "bad [/noparse]"}
        with self.assertRaisesRegex(ValueError, "noparse closing tag"):
            reference_payload(source, "safe")


if __name__ == "__main__":
    unittest.main()
