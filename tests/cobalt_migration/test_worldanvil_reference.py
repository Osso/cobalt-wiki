"""Exact source storage in native Author's Notes, without body-renderer changes."""

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
        self.assertEqual(payload["title"], "Source reference: template:infobox")
        self.assertEqual(payload["templateType"], "article")
        self.assertEqual(payload["state"], "private")
        self.assertIn("Infobox", payload["content"])
        self.assertIn("template:infobox", payload["content"])
        self.assertIn("Author's Notes", payload["content"])
        self.assertEqual(
            payload["tags"].split(","),
            [
                "reference",
                "wikidot",
                "source-reference",
                "cobalt-source:template:infobox",
            ],
        )

    def test_source_is_verbatim_in_authornotes_not_lossy_body(self):
        raw = '/* comment */\n     padding: 0px;\n[[code type="css"]]\n<widget attr="x&y">\n[[/code]]\n'
        payload = reference_payload(SOURCE, raw)
        self.assertEqual(payload["authornotes"], raw)
        self.assertNotIn(raw, payload["content"])
        self.assertNotIn("[code]", payload["content"])

    def test_literal_parser_delimiters_are_safe_in_stored_source(self):
        raw = "before [/noparse] [code]x[/code] [/NoParse] after"
        self.assertEqual(reference_payload(SOURCE, raw)["authornotes"], raw)

    def test_reference_identity_does_not_require_unavailable_metadata_title(self):
        source = {"fullname": "player:_public", "tags": [], "status": "denied"}
        payload = reference_payload(source, "Exact supplied backup source")
        self.assertEqual(payload["title"], "Source reference: player:_public")
        self.assertIn(
            "Original title unavailable in metadata export", payload["content"]
        )
        self.assertEqual(payload["authornotes"], "Exact supplied backup source")

    def test_source_identity_cannot_close_metadata_literal_wrapper(self):
        with self.assertRaisesRegex(ValueError, "noparse closing tag"):
            reference_payload({**SOURCE, "title": "bad [/noparse]"}, "safe")


if __name__ == "__main__":
    unittest.main()
