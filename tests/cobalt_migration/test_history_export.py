"""Synthetic authenticated module responses; no private source content."""

import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

from tools.cobalt_migration.history_export import (
    HistoryExportError,
    HistoryResponse,
    export_history,
)


def revision(number, identity):
    return (
        f'<tr id="revision-row-{identity}"><td>{number}.</td><td></td>'
        '<td><span class="spantip" title="source">S</span></td>'
        f'<td><a onclick="showSource({identity})">Source</a></td>'
        '<td></td><td><span class="odate time_1600000000">date</span>'
        "<td>comment</td></td></tr>"
    )


def listing(rows, page=1, previous=None, next_page=None):
    links = "".join(
        f'<a onclick="updatePagedList({target})">{label}</a>'
        for target, label in ((previous, "« previous"), (next_page, "next »"))
        if target is not None
    )
    return (
        f'<div class="pager"><span class="current">{page}</span>{links}</div>'
        f'<div class="page-history"><table>{"".join(rows)}</table></div>'
    )


def source(text):
    return f'<div class="page-source">\n{text}\n</div>'


class HistoryExportTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "private" / "history"
        self.origin = "https://example.test"
        self.page_id = 123
        self.requests = []
        self.responses = {
            ("history/PageRevisionListModule", 1): listing(
                [revision(3, 103), revision(2, 102)], next_page=2
            ),
            ("history/PageRevisionListModule", 2): listing(
                [revision(0, 100)], page=2, previous=1
            ),
            ("history/PageSourceModule", 103): source("three"),
            ("history/PageSourceModule", 102): source("two"),
            ("history/PageSourceModule", 100): source("zero"),
        }

    def fetch(self, request):
        self.requests.append(request)
        if request["moduleName"] == "history/PageRevisionListModule":
            self.assertEqual(
                request,
                {
                    "moduleName": "history/PageRevisionListModule",
                    "page": 1
                    if len(
                        [
                            r
                            for r in self.requests
                            if r["moduleName"] == "history/PageRevisionListModule"
                        ]
                    )
                    == 1
                    else 2,
                    "perpage": 20,
                    "page_id": 123,
                    "options": {"all": True},
                },
            )
            key = (request["moduleName"], request["page"])
        else:
            self.assertEqual(set(request), {"moduleName", "revision_id"})
            key = (request["moduleName"], request["revision_id"])
        return HistoryResponse(200, f"RAW:{key[1]}", self.responses[key])

    def export(self, fetch=None, **kwargs):
        return export_history(
            self.origin,
            self.page_id,
            self.path,
            fetch or self.fetch,
            sleep=lambda _: None,
            jitter=lambda: 0,
            now=lambda: 1234,
            **kwargs,
        )

    def test_complete_inventory_before_bodies_and_resume_without_repeating_requests(
        self,
    ):
        def interrupted(request):
            if (
                request["moduleName"] == "history/PageSourceModule"
                and request["revision_id"] == 102
            ):
                raise RuntimeError("interrupted")
            return self.fetch(request)

        with self.assertRaisesRegex(RuntimeError, "interrupted"):
            self.export(interrupted)
        self.assertEqual(
            [r["moduleName"] for r in self.requests],
            ["history/PageRevisionListModule"] * 2 + ["history/PageSourceModule"],
        )
        state = self.export()
        self.assertEqual([r["revision_id"] for r in self.requests[3:]], [102, 100])
        self.assertEqual([r["number"] for r in state["revisions"]], [3, 2, 0])
        self.assertEqual(state["unobserved_ranges"], [[1, 1]])
        self.assertEqual(
            [state["bodies"][str(r)]["acquired_at"] for r in (103, 102, 100)],
            [1234] * 3,
        )
        self.assertEqual(
            [page["acquired_at"] for page in state["list_pages"]], [1234, 1234]
        )
        self.assertEqual((self.path / "list-2.raw").read_text(), "RAW:2")
        self.assertEqual((self.path / "revision-100.html").read_text(), source("zero"))
        self.assertEqual(state["bodies"]["100"]["wikitext"], "zero")
        self.assertEqual(
            state["bodies"]["100"]["representation"], "display-decoded-not-byte-exact"
        )
        self.assertEqual(stat.S_IMODE(self.path.stat().st_mode), 0o700)
        self.assertTrue(
            all(stat.S_IMODE(p.stat().st_mode) == 0o600 for p in self.path.iterdir())
        )
        self.export(lambda _: self.fail("completed archive made another request"))

    def test_resume_after_list_interruption_before_any_body(self):
        def interrupted(request):
            if (
                request["moduleName"] == "history/PageRevisionListModule"
                and request["page"] == 2
            ):
                raise RuntimeError("interrupted")
            return self.fetch(request)

        with self.assertRaisesRegex(RuntimeError, "interrupted"):
            self.export(interrupted)
        self.assertEqual(len(self.requests), 1)
        self.export()
        self.assertEqual([r["page"] for r in self.requests if "page" in r], [1, 2])

    def test_resume_after_final_list_checkpoint_without_refetch(self):
        from tools.cobalt_migration.listing_export import _save_checkpoint

        def stop_before_inventory(path, state):
            if state["revisions"] is not None:
                raise RuntimeError("interrupted after last list")
            _save_checkpoint(path, state)

        with patch(
            "tools.cobalt_migration.history_export._save_checkpoint",
            stop_before_inventory,
        ):
            with self.assertRaisesRegex(RuntimeError, "interrupted after last list"):
                self.export()
        self.assertEqual([r["page"] for r in self.requests], [1, 2])
        self.export()
        self.assertEqual([r["page"] for r in self.requests if "page" in r], [1, 2])

    def test_duplicate_or_inconsistent_lists_stop_before_bodies(self):
        for second in (
            listing([revision(2, 999)], page=2, previous=1),
            listing([revision(1, 103)], page=2, previous=1),
            listing([revision(1, 101)], page=3, previous=2),
        ):
            with (
                self.subTest(second=second),
                tempfile.TemporaryDirectory() as directory,
            ):
                self.path = Path(directory) / "private"
                self.responses[("history/PageRevisionListModule", 2)] = second
                self.requests.clear()
                with self.assertRaises(HistoryExportError):
                    self.export()
                self.assertFalse(
                    any(
                        r["moduleName"] == "history/PageSourceModule"
                        for r in self.requests
                    )
                )

    def test_permanent_denial_and_body_gap_are_explicit(self):
        def denied(request):
            if request["moduleName"] == "history/PageRevisionListModule":
                return HistoryResponse(200, "denied-raw", "Permission denied")
            return self.fetch(request)

        with self.assertRaises(HistoryExportError):
            self.export(denied)
        self.assertFalse((self.path / "checkpoint.json").exists())
        self.assertEqual(len(self.requests), 0)
        self.path = Path(self.temp.name) / "other" / "history"

        def missing(request):
            if request.get("revision_id") == 102:
                self.requests.append(request)
                return HistoryResponse(404, "not-found-raw", "Not found")
            return self.fetch(request)

        state = self.export(missing)
        self.assertEqual(state["bodies"]["102"]["status"], 404)
        self.assertNotIn("wikitext", state["bodies"]["102"])
        self.assertEqual((self.path / "revision-102.raw").read_text(), "not-found-raw")

    def test_transient_retries_spacing_and_exhaustion_preserve_checkpoint(self):
        waits = []
        attempts = 0

        def flaky(request):
            nonlocal attempts
            attempts += 1
            if attempts == 1:
                raise TimeoutError()
            if attempts == 2:
                return HistoryResponse(429, "busy", "busy", "5")
            return self.fetch(request)

        export_history(
            self.origin,
            self.page_id,
            self.path,
            flaky,
            sleep=waits.append,
            jitter=lambda: 0,
            now=lambda: 1234,
        )
        self.assertEqual(waits[:2], [1, 5.0])
        self.assertEqual(waits[2:], [1] * 4)
        self.assertEqual(attempts, 7)
        self.path = Path(self.temp.name) / "exhausted"
        count = 0

        def unavailable(request):
            nonlocal count
            count += 1
            return HistoryResponse(503, "failure", "failure")

        with self.assertRaisesRegex(HistoryExportError, "four attempts"):
            self.export(unavailable)
        self.assertEqual(count, 4)
        self.assertFalse((self.path / "checkpoint.json").exists())

    def test_html_denial_in_body_stops_without_advancing(self):
        def denied(request):
            if request.get("revision_id") == 102:
                return HistoryResponse(200, "denied-raw", "Permission denied")
            return self.fetch(request)

        with self.assertRaises(HistoryExportError):
            self.export(denied)
        state = json.loads((self.path / "checkpoint.json").read_text())
        self.assertEqual(set(state["bodies"]), {"103"})
        self.assertFalse((self.path / "revision-102.raw").exists())

    def test_checkpoint_identity_and_corrupt_archive_refuse_resume(self):
        self.export()
        with self.assertRaises(HistoryExportError):
            export_history(
                "https://other.test",
                self.page_id,
                self.path,
                lambda _: self.fail("identity changed"),
            )
        with self.assertRaises(HistoryExportError):
            export_history(
                self.origin, 124, self.path, lambda _: self.fail("identity changed")
            )
        (self.path / "revision-103.raw").write_text("tampered")
        with self.assertRaises(HistoryExportError):
            self.export(lambda _: self.fail("corrupt archive must not fetch"))


if __name__ == "__main__":
    unittest.main()
