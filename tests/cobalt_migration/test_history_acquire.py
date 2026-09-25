"""Behavioral site acquisition tests using concrete source inventories."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.cobalt_migration.history_acquire import acquire_site_history
from tools.cobalt_migration.history_export import HistoryExportError


class Clock:
    def __init__(self):
        self.current = 0.0

    def monotonic(self):
        return self.current

    def sleep(self, seconds):
        self.current += seconds


class SiteAcquisitionTest(unittest.TestCase):
    def test_fetch_spacing_spans_pages_from_previous_completion(self):
        plan = {
            "pages": [
                {"metadata_status": "accepted", "metadata": {"page_id": page_id}}
                for page_id in (11, 22)
            ]
        }
        clock = Clock()
        observed = []

        def fetch(request):
            observed.append((request["page_id"], clock.monotonic()))
            clock.sleep(0.25)
            return {"status": 200}

        def exporter(origin, page_id, directory, paced_fetch):
            paced_fetch({"page_id": page_id})
            if page_id == 11:
                clock.sleep(1.5)  # Work after first request already satisfies spacing.
            paced_fetch({"page_id": page_id})
            return {"revisions": [], "bodies": {}, "unobserved_ranges": []}

        with tempfile.TemporaryDirectory() as temporary:
            with (
                patch("time.monotonic", clock.monotonic),
                patch("time.sleep", clock.sleep),
            ):
                acquire_site_history(
                    plan,
                    "https://example.test",
                    Path(temporary) / "history",
                    fetch,
                    exporter=exporter,
                )
        self.assertEqual(observed, [(11, 0.0), (11, 1.75), (22, 3.0), (22, 4.25)])

    def test_elapsed_page_work_avoids_extra_boundary_wait(self):
        plan = {
            "pages": [
                {"metadata_status": "accepted", "metadata": {"page_id": page_id}}
                for page_id in (11, 22)
            ]
        }
        clock = Clock()
        observed = []

        def fetch(request):
            observed.append((request["page_id"], clock.monotonic()))
            clock.sleep(0.25)
            return {"status": 200}

        def exporter(origin, page_id, directory, paced_fetch):
            paced_fetch({"page_id": page_id})
            if page_id == 11:
                clock.sleep(1.5)
            return {"revisions": [], "bodies": {}, "unobserved_ranges": []}

        with tempfile.TemporaryDirectory() as temporary:
            with (
                patch("time.monotonic", clock.monotonic),
                patch("time.sleep", clock.sleep),
            ):
                acquire_site_history(
                    plan,
                    "https://example.test",
                    Path(temporary) / "history",
                    fetch,
                    exporter=exporter,
                )
        self.assertEqual(observed, [(11, 0.0), (22, 1.75)])
        self.assertEqual(clock.monotonic(), 2.0)

    def test_cached_page_does_not_wait_without_fetch(self):
        plan = {
            "pages": [
                {"metadata_status": "accepted", "metadata": {"page_id": page_id}}
                for page_id in (11, 22)
            ]
        }
        clock = Clock()
        observed = []

        def fetch(request):
            observed.append((request["page_id"], clock.monotonic()))
            return {"status": 200}

        def exporter(origin, page_id, directory, paced_fetch):
            if page_id == 22:
                paced_fetch({"page_id": page_id})
            return {"revisions": [], "bodies": {}, "unobserved_ranges": []}

        with tempfile.TemporaryDirectory() as temporary:
            with (
                patch("time.monotonic", clock.monotonic),
                patch("time.sleep", clock.sleep),
            ):
                acquire_site_history(
                    plan,
                    "https://example.test",
                    Path(temporary) / "history",
                    fetch,
                    exporter=exporter,
                )
        self.assertEqual(observed, [(22, 0.0)])
        self.assertEqual(clock.monotonic(), 0.0)

    def test_fetch_exception_propagates_without_retry(self):
        plan = {"pages": [{"metadata_status": "accepted", "metadata": {"page_id": 11}}]}
        failure = RuntimeError("source fetch failed")
        observed = []

        def fetch(request):
            observed.append(request)
            raise failure

        def exporter(origin, page_id, directory, paced_fetch):
            paced_fetch({"page_id": page_id})

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "history"
            with self.assertRaises(RuntimeError) as caught:
                acquire_site_history(
                    plan, "https://example.test", directory, fetch, exporter=exporter
                )
            self.assertIs(caught.exception, failure)
            self.assertEqual(observed, [{"page_id": 11}])
            self.assertEqual(
                json.loads((directory / "site-progress.json").read_text())[
                    "failed_page_id"
                ],
                11,
            )

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
        with (
            tempfile.TemporaryDirectory() as temporary,
            self.assertRaisesRegex(ValueError, "duplicate"),
        ):
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
