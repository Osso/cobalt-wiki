"""Behavioral tests for additive World Anvil import decisions and checkpoints."""

import json
import tempfile
import unittest
from pathlib import Path

from tools.cobalt_migration.worldanvil_import import (
    ImportBlocked,
    import_page,
    index_articles,
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

    def create_article(self, world_id, payload):
        article_id = NEW_ID if NEW_ID not in self.articles else "second-created-id"
        article = {**payload, "id": article_id, "world": {"id": world_id}}
        self.articles[article_id] = article
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
                remote,
                WORLD,
                SOURCE,
                player_payload(SOURCE, FIELDS),
                journal,
                index_articles(remote.articles.values()),
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
                index_articles(remote.articles.values()),
            )
            self.assertEqual(result["status"], "existing")
            self.assertEqual(remote.articles, {"old-id": old})

    def test_application_is_not_confused_with_existing_player_of_same_name(self):
        old = {
            "id": "aly-player",
            "title": "Aly",
            "tags": "player,aly",
            "content": "Manual player biography",
        }
        remote = Destination([old])
        source = {"fullname": "application:aly", "title": "Aly"}
        payload = {
            "title": "Application: Aly",
            "templateType": "article",
            "state": "private",
            "content": "Original application",
            "tags": "cobalt-source:application:aly",
        }
        with tempfile.TemporaryDirectory() as directory:
            result = import_page(
                remote,
                WORLD,
                source,
                payload,
                Path(directory) / "journal.json",
                index_articles(remote.articles.values()),
            )
            self.assertEqual(result["status"], "created")
            self.assertEqual(remote.articles["aly-player"], old)
            self.assertEqual(remote.articles[NEW_ID]["title"], "Application: Aly")

    def test_lost_creation_response_is_durably_blocked_on_resume(self):
        remote = Destination(lose_response=True)
        inventory = index_articles(remote.articles.values())
        with tempfile.TemporaryDirectory() as directory:
            journal = Path(directory) / "journal.json"
            with self.assertRaises(TimeoutError):
                import_page(
                    remote,
                    WORLD,
                    SOURCE,
                    player_payload(SOURCE, FIELDS),
                    journal,
                    inventory,
                )
            self.assertEqual(len(remote.articles), 1)
            self.assertEqual(
                json.loads(journal.read_text())["pages"]["player:anakin"]["status"],
                "pending",
            )
            with self.assertRaises(ImportBlocked):
                import_page(
                    remote,
                    WORLD,
                    SOURCE,
                    player_payload(SOURCE, FIELDS),
                    journal,
                    inventory,
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
        inventory = index_articles(remote.articles.values())
        with tempfile.TemporaryDirectory() as directory:
            journal = Path(directory) / "journal.json"
            with self.assertRaises(ImportBlocked):
                import_page(
                    remote,
                    WORLD,
                    SOURCE,
                    player_payload(SOURCE, FIELDS),
                    journal,
                    inventory,
                )
            self.assertEqual(
                json.loads(journal.read_text())["pages"]["player:anakin"]["status"],
                "created_unverified",
            )
            duplicate = {"fullname": "player:ani", "title": "Anakin"}
            result = import_page(
                remote, WORLD, duplicate, {"title": "Anakin"}, journal, inventory
            )
            self.assertEqual(result["status"], "existing")
            self.assertEqual(len(remote.articles), 1)

    def test_batch_creates_two_pages_then_blocks_duplicate_alias(self):
        remote = Destination()
        inventory = index_articles(remote.articles.values())
        with tempfile.TemporaryDirectory() as directory:
            journal = Path(directory) / "journal.json"
            for name in ("Anakin", "Bail"):
                source = {"fullname": f"player:{name.lower()}", "title": name}
                payload = player_payload(source, FIELDS)
                result = import_page(remote, WORLD, source, payload, journal, inventory)
                self.assertEqual(result["status"], "created")
            alias = {"fullname": "player:alternate", "title": "ANAKIN"}
            result = import_page(
                remote, WORLD, alias, {"title": alias["title"]}, journal, inventory
            )
            self.assertEqual(result["status"], "existing")
            self.assertEqual(len(remote.articles), 2)
            self.assertEqual(remote.articles[NEW_ID]["title"], "Anakin")
            self.assertEqual(remote.articles["second-created-id"]["title"], "Bail")

    def test_refreshed_snapshot_finds_new_source_marker_without_name_match(self):
        remote = Destination()
        inventory = index_articles(remote.articles.values())
        article = {
            "id": "manual-id",
            "title": "A Different Heading",
            "slug": "different",
            "tags": "notes,cobalt-source:player:anakin",
            "content": "Manual notes",
        }
        remote.articles["manual-id"] = article
        inventory = index_articles(remote.articles.values())
        with tempfile.TemporaryDirectory() as directory:
            result = import_page(
                remote,
                WORLD,
                SOURCE,
                player_payload(SOURCE, FIELDS),
                Path(directory) / "journal.json",
                inventory,
            )
            self.assertEqual(result["status"], "existing")
            self.assertEqual(result["id"], "manual-id")
            self.assertEqual(remote.articles, {"manual-id": article})

    def test_ambiguous_title_and_slug_candidates_block_create(self):
        articles = [
            {"id": "manual-a", "title": "Anakin", "content": "First notes"},
            {
                "id": "manual-b",
                "title": "Other",
                "slug": "anakin",
                "content": "Second notes",
            },
        ]
        remote = Destination(articles)
        inventory = index_articles(remote.articles.values())
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(
                ImportBlocked, "multiple live identity candidates"
            ):
                import_page(
                    remote,
                    WORLD,
                    SOURCE,
                    player_payload(SOURCE, FIELDS),
                    Path(directory) / "journal.json",
                    inventory,
                )
            self.assertEqual(remote.articles, {a["id"]: a for a in articles})


if __name__ == "__main__":
    unittest.main()
