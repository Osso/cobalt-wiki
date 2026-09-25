"""Behavioral tests for additive World Anvil import decisions and checkpoints."""

import json
import tempfile
import unittest
from pathlib import Path

from tools.cobalt_migration.worldanvil_import import (
    ImportBlocked,
    import_page,
    player_payload,
)

WORLD = "fdaa8418-78bd-4c72-9f51-7c837719ea93"
NEW_ID = "c655b388-ac19-4aeb-9878-8522cdff51da"
SOURCE = {"fullname": "player:anakin", "title": "Anakin", "tags": ["player", "anakin"]}
FIELDS = {
    "portrait": "",
    "nicknames": "Ani",
    "pronouns": "they/them",
    "battleTag": "Example#1234",
    "discordUsername": "example",
    "timezone": "Eastern",
    "whoAmI": "I enjoy stories.\n\nAnd gardening.",
    "rpPrefs": "Lighthearted stories.",
    "contactPrefs": "Discord is good!",
}


class Destination:
    """An observable remote article store, not a method-call expectation."""

    def __init__(self, articles=(), lose_response=False):
        self.articles = {a["id"]: dict(a) for a in articles}
        self.lose_response = lose_response

    def list_articles(self, world_id):
        return list(self.articles.values())

    def create_article(self, world_id, payload):
        article = {**payload, "id": NEW_ID, "world": {"id": world_id}}
        self.articles[NEW_ID] = article
        if self.lose_response:
            raise TimeoutError("response lost after remote creation")
        return article

    def get_article(self, article_id):
        return dict(self.articles[article_id])


class PlayerPayloadTests(unittest.TestCase):
    def test_preserves_profile_text_and_keeps_contact_information_private(self):
        payload = player_payload(SOURCE, FIELDS)
        self.assertEqual(payload["title"], "Anakin")
        self.assertEqual(payload["templateType"], "article")
        self.assertEqual(payload["state"], "private")
        self.assertFalse(payload["isDraft"])
        self.assertIn(
            "[p]I enjoy stories.[/p]\n[p]And gardening.[/p]", payload["content"]
        )
        self.assertIn(
            "[h1]Contact Preferences[/h1]\n[p]Discord is good![/p]", payload["content"]
        )
        self.assertIn("--Pronouns::they/them--", payload["sidepanelcontenttop"])
        self.assertIn("--BattleTag::Example#1234--", payload["sidepanelcontenttop"])
        self.assertIn("cobalt-source:player:anakin", payload["tags"].split(","))

    def test_unresolved_portrait_or_wiki_markup_is_not_silently_dropped(self):
        for changes in (
            {"portrait": "portrait.png"},
            {"whoAmI": "[[include AboutMe]]"},
        ):
            with self.subTest(changes=changes), self.assertRaises(ImportBlocked):
                player_payload(SOURCE, {**FIELDS, **changes})

    def test_image_id_precedes_sidebar_definitions(self):
        fields = {**FIELDS, "portrait": "portrait.png"}
        payload = player_payload(SOURCE, fields, portrait=6815014)
        self.assertEqual(
            payload["sidepanelcontenttop"].splitlines()[0:2],
            ["[img:6815014|none]", "--Nicknames::Ani--"],
        )
        self.assertNotIn("[img:", payload["content"])

    def test_external_image_url_must_match_source_exactly(self):
        for url in (
            "https://example.org/portraits/Aze_Veil.jpg",
            "http://cobalt-company.wdfiles.com/local--files/character:aszera/Aze_Veil.jpg",
        ):
            with self.subTest(url=url):
                payload = player_payload(
                    SOURCE, {**FIELDS, "portrait": url}, portrait=url
                )
                self.assertEqual(
                    payload["sidepanelcontenttop"].splitlines()[0], f"[img:{url}|none]"
                )

    def test_missing_portrait_cannot_gain_an_image(self):
        for reference in (6815014, "https://example.org/portrait.jpg"):
            with self.subTest(reference=reference), self.assertRaises(ImportBlocked):
                player_payload(SOURCE, FIELDS, portrait=reference)

    def test_invalid_portrait_references_block_conversion(self):
        source_url = "http://example.org/portrait.jpg"
        fields = {**FIELDS, "portrait": source_url}
        for reference in (
            True,
            False,
            0,
            -1,
            1.5,
            "6815014",
            "https://example.org/portrait.jpg",
            "ftp://example.org/portrait.jpg",
            "http://example.org/portrait.jpg|right",
            "http://example.org/portrait.jpg[bad]",
        ):
            with self.subTest(reference=reference), self.assertRaises(ImportBlocked):
                player_payload(SOURCE, fields, portrait=reference)

    def test_matching_external_url_with_bbcode_delimiters_blocks_conversion(self):
        for url in (
            "https://example.org/a|b.jpg",
            "https://example.org/a[b].jpg",
            "https://example.org/a]b.jpg",
        ):
            with self.subTest(url=url), self.assertRaises(ImportBlocked):
                player_payload(SOURCE, {**FIELDS, "portrait": url}, portrait=url)

    def test_plain_profile_url_is_retained_without_treating_scheme_as_italics(self):
        fields = {
            **FIELDS,
            "whoAmI": "Artist and storyteller.\nhttps://erzahlerin.myportfolio.com/",
        }
        payload = player_payload(SOURCE, fields)
        self.assertIn("https://erzahlerin.myportfolio.com/", payload["content"])
        with self.assertRaises(ImportBlocked):
            player_payload(SOURCE, {**FIELDS, "whoAmI": "//Actual italic text//"})

    def test_unknown_nonempty_fields_block_instead_of_losing_data(self):
        with self.assertRaises(ImportBlocked):
            player_payload(SOURCE, {**FIELDS, "extraBiography": "Keep this too"})


class ImportPageTests(unittest.TestCase):
    def test_create_preserves_existing_article_and_records_readback(self):
        old = {"id": "old-id", "title": "Alli", "content": "Manually edited"}
        remote = Destination([old])
        with tempfile.TemporaryDirectory() as directory:
            journal = Path(directory) / "journal.json"
            result = import_page(
                remote, WORLD, SOURCE, player_payload(SOURCE, FIELDS), journal
            )
            self.assertEqual(result["status"], "created")
            self.assertEqual(remote.articles["old-id"], old)
            self.assertEqual(remote.articles[NEW_ID]["title"], "Anakin")
            self.assertEqual(
                json.loads(journal.read_text())["pages"]["player:anakin"]["id"], NEW_ID
            )

    def test_existing_same_name_is_preserved_without_creation(self):
        old = {"id": "old-id", "title": "ANAKIN", "content": "Manually edited"}
        remote = Destination([old])
        with tempfile.TemporaryDirectory() as directory:
            result = import_page(
                remote,
                WORLD,
                SOURCE,
                player_payload(SOURCE, FIELDS),
                Path(directory) / "journal.json",
            )
            self.assertEqual(result["status"], "existing")
            self.assertEqual(remote.articles, {"old-id": old})

    def test_lost_creation_response_is_durably_blocked_on_resume(self):
        remote = Destination(lose_response=True)
        with tempfile.TemporaryDirectory() as directory:
            journal = Path(directory) / "journal.json"
            with self.assertRaises(TimeoutError):
                import_page(
                    remote, WORLD, SOURCE, player_payload(SOURCE, FIELDS), journal
                )
            self.assertEqual(len(remote.articles), 1)
            self.assertEqual(
                json.loads(journal.read_text())["pages"]["player:anakin"]["status"],
                "pending",
            )
            with self.assertRaises(ImportBlocked):
                import_page(
                    remote, WORLD, SOURCE, player_payload(SOURCE, FIELDS), journal
                )
            self.assertEqual(len(remote.articles), 1)

    def test_readback_mismatch_does_not_mark_import_verified(self):
        class CorruptDestination(Destination):
            def get_article(self, article_id):
                return {
                    **super().get_article(article_id),
                    "content": "Template placeholder",
                }

        remote = CorruptDestination()
        with tempfile.TemporaryDirectory() as directory:
            journal = Path(directory) / "journal.json"
            with self.assertRaises(ImportBlocked):
                import_page(
                    remote, WORLD, SOURCE, player_payload(SOURCE, FIELDS), journal
                )
            self.assertEqual(
                json.loads(journal.read_text())["pages"]["player:anakin"]["status"],
                "created_unverified",
            )


if __name__ == "__main__":
    unittest.main()
