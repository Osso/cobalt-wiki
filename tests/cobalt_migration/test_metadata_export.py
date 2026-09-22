import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from tools.cobalt_migration import metadata_export as export
from tools.cobalt_migration.listing_export import FetchResponse, ListingExportError
from tools.cobalt_migration.page_metadata import PageMetadataError
from tests.cobalt_migration.test_page_metadata import page_html

ORIGIN = "https://example.test"
NAMES = ["character:a_b", "writing:é"]


def page(name, page_id=42):
    return FetchResponse(
        200,
        page_html(
            assignments=(
                f"WIKIREQUEST.info.pageUnixName = {json.dumps(name)};"
                f"WIKIREQUEST.info.pageId = {page_id};"
            )
        ),
    )


class MetadataExportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "private" / "metadata.json"
        self.calls = []
        self.delays = []

    def acquire(self, outcomes, names=NAMES, origin=ORIGIN):
        def fetch(path):
            self.calls.append(path)
            result = outcomes.pop(0)
            if isinstance(result, Exception):
                raise result
            return result

        return export.export_metadata(
            origin,
            names,
            self.path,
            fetch,
            sleep=self.delays.append,
            jitter=lambda: 0,
            now=lambda: 0,
        )

    def test_two_pages_exact_metadata_and_completed_resume(self):
        state = self.acquire([page(NAMES[0]), page(NAMES[1], 43)])
        self.assertEqual(self.calls, ["/character:a_b", "/writing:%C3%A9"])
        self.assertEqual(self.delays, [1])
        self.assertEqual(state["completed_position"], 2)
        self.assertEqual(
            state["records"][0],
            {
                "status": "accepted",
                "archive_key": "character_a_b",
                "fullname": "character:a_b",
                "page_id": 42,
                "title": "Sir Dane & Atley",
                "tags": ["_completed", "human"],
                "revision_number": 407,
                "updated_at": 1755828499,
            },
        )
        self.assertEqual(state["records"][1]["page_id"], 43)
        self.assertEqual(self.acquire([]), state)
        self.assertEqual(json.loads(self.path.read_text()), state)
        self.assertEqual(
            set(state),
            {
                "schema",
                "source_origin",
                "names_sha256",
                "completed_position",
                "records",
            },
        )
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(self.path.parent.stat().st_mode & 0o777, 0o700)

    def test_failure_then_resume_current_page(self):
        with self.assertRaises(ListingExportError):
            self.acquire([page(NAMES[0]), FetchResponse(403, "private")])
        self.assertEqual(json.loads(self.path.read_text())["completed_position"], 1)
        state = self.acquire([page(NAMES[1])])
        self.assertEqual(
            self.calls, ["/character:a_b", "/writing:%C3%A9", "/writing:%C3%A9"]
        )
        self.assertEqual(state["completed_position"], 2)

    def test_digest_and_origin_mismatch_before_fetch(self):
        self.acquire([page(NAMES[0]), page(NAMES[1])])
        for names, origin in [
            (list(reversed(NAMES)), ORIGIN),
            (NAMES, "https://other.test"),
        ]:
            with self.subTest(names=names, origin=origin):
                with self.assertRaises(export.MetadataExportError):
                    self.acquire([], names=names, origin=origin)
        self.assertEqual(len(self.calls), 2)

    def test_denied_and_missing_are_explicit_not_accepted(self):
        state = self.acquire(
            [
                FetchResponse(200, '<h1 id="page-title">Permission denied</h1>'),
                FetchResponse(200, '<h1 id="page-title">Page not found</h1>'),
            ]
        )
        self.assertEqual(
            state["records"],
            [
                {
                    "fullname": NAMES[0],
                    "archive_key": "character_a_b",
                    "status": "denied",
                },
                {
                    "fullname": NAMES[1],
                    "archive_key": "writing_é",
                    "status": "not_found",
                },
            ],
        )
        self.assertEqual(state["completed_position"], 2)

    def test_identity_or_parser_failure_does_not_advance(self):
        for response in [page("wrong:identity"), FetchResponse(200, "broken")]:
            with self.subTest(response=response):
                with self.assertRaises(PageMetadataError):
                    self.acquire([response])
                self.assertFalse(self.path.exists())
        self.assertEqual(len(self.calls), 2)

    def test_transient_retries_respect_retry_after(self):
        state = self.acquire(
            [
                ConnectionError("secret"),
                FetchResponse(429, "", "7"),
                FetchResponse(503, ""),
                page(NAMES[0]),
                page(NAMES[1]),
            ]
        )
        self.assertEqual(self.delays, [1, 7, 4, 1])
        self.assertEqual(state["completed_position"], 2)

    def test_retry_exhaustion_and_unexpected_transport_do_not_advance(self):
        for outcomes, error in [
            ([TimeoutError()] * 4, ListingExportError),
            ([RuntimeError("transport failed")], RuntimeError),
        ]:
            with self.subTest(error=error):
                with self.assertRaises(error):
                    self.acquire(outcomes)
                self.assertFalse(self.path.exists())

    def test_atomic_failure_preserves_previous_record(self):
        with self.assertRaises(ListingExportError):
            self.acquire([page(NAMES[0]), FetchResponse(404, "")])
        before = self.path.read_bytes()
        with patch(
            "tools.cobalt_migration.listing_export.os.replace",
            side_effect=OSError("full"),
        ):
            with self.assertRaises(OSError):
                self.acquire([page(NAMES[1])])
        self.assertEqual(self.path.read_bytes(), before)
        self.assertEqual(list(self.path.parent.iterdir()), [self.path])

    def test_unprotected_parent_rejected_before_fetch(self):
        self.path.parent.mkdir(mode=0o755)
        self.path.parent.chmod(0o755)
        with self.assertRaises(ListingExportError):
            self.acquire([])
        self.assertEqual(self.calls, [])

    def test_invalid_names_and_archive_collisions_rejected(self):
        for names in [["a", "a"], ["a:b", "a_b"], [""], ["a/b"]]:
            with self.subTest(names=names):
                with self.assertRaises(export.MetadataExportError):
                    self.acquire([], names=names)
        self.assertEqual(self.calls, [])

    def test_corrupt_position_rejected_before_fetch(self):
        state = self.acquire([page(NAMES[0]), page(NAMES[1])])
        state["completed_position"] = 1
        self.path.write_text(json.dumps(state))
        with self.assertRaises(export.MetadataExportError):
            self.acquire([])
        self.assertEqual(len(self.calls), 2)


if __name__ == "__main__":
    unittest.main()
