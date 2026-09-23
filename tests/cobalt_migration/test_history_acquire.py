"""Behavioral site acquisition tests using concrete source inventories."""

import json
from pathlib import Path
import tempfile
import unittest

from tools.cobalt_migration.history_acquire import acquire_site_history
from tools.cobalt_migration.history_export import HistoryExportError


class SiteAcquisitionTest(unittest.TestCase):
    def test_multiple_pages_resume_and_unresolved_metadata_remains_explicit(self):
        plan = {
            "pages": [
                {"metadata_status": "accepted", "metadata": {"page_id": 11}},
                {"metadata_status": "denied", "metadata": {"status": "denied"}},
                {"metadata_status": "accepted", "metadata": {"page_id": 22}},
            ]
        }
        calls = []
        fail = True

        def exporter(origin, page_id, directory, fetch):
            nonlocal fail
            calls.append(page_id)
            if page_id == 22 and fail:
                fail = False
                raise HistoryExportError("transient failure exhausted")
            return {
                "revisions": [{"revision_id": page_id}],
                "bodies": {str(page_id): {"status": 200}},
                "unobserved_ranges": [],
            }

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "history"
            with self.assertRaises(HistoryExportError):
                acquire_site_history(
                    plan, "https://example.test", directory, None, exporter=exporter
                )
            partial = json.loads((directory / "site-progress.json").read_text())
            self.assertEqual(partial["pages"]["11"]["bodies"], 1)
            self.assertEqual(partial["failed_page_id"], 22)
            self.assertEqual(
                partial["unresolved_metadata"], [{"position": 1, "status": "denied"}]
            )
            result = acquire_site_history(
                plan, "https://example.test", directory, None, exporter=exporter
            )
            self.assertEqual(calls, [11, 22, 11, 22])
            self.assertEqual(len(result["pages"]), 2)
            self.assertIsNone(result["failed_page_id"])
            self.assertEqual(result["listed_revisions"], 2)
            self.assertEqual(result["acquired_bodies"], 2)

    def test_duplicate_source_ids_fail_before_fetch(self):
        plan = {
            "pages": [{"metadata_status": "accepted", "metadata": {"page_id": 11}}] * 2
        }
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(ValueError, "duplicate"):
                acquire_site_history(
                    plan, "https://example.test", Path(temporary) / "history", None
                )

    def test_origin_or_inventory_change_cannot_reuse_archive(self):
        plan = {"pages": [{"metadata_status": "accepted", "metadata": {"page_id": 11}}]}

        def exporter(*args):
            return {"revisions": [], "bodies": {}, "unobserved_ranges": []}

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "history"
            acquire_site_history(
                plan, "https://example.test", directory, None, exporter=exporter
            )
            with self.assertRaisesRegex(ValueError, "identity"):
                acquire_site_history(
                    plan, "https://other.test", directory, None, exporter=exporter
                )


if __name__ == "__main__":
    unittest.main()
