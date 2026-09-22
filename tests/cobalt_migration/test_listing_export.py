import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from tools.cobalt_migration import listing_export as export

ORIGIN = "https://example.test"


def listing(names, highest=2):
    links = "".join(f'<a href="/{name}">{name}</a>' for name in names)
    return (
        f'<div id="page-content"><div class="list-pages-box">{links}'
        f'<div class="pager"><a href="/pagelist/p/{highest}">last</a>'
        "</div></div></div>"
    )


class ListingExportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "private" / "listing.json"
        self.calls = []
        self.delays = []

    def run_export(self, outcomes, **kwargs):
        def fetch(path):
            self.calls.append(path)
            value = outcomes.pop(0)
            if isinstance(value, Exception):
                raise value
            return value

        return export.export_listing(
            ORIGIN, self.path, fetch, sleep=self.delays.append, **kwargs
        )

    def test_pages_deduplicate_literal_names_and_finished_resume(self):
        result = self.run_export(
            [
                export.FetchResponse(200, listing(["character:a_b", "_hidden"])),
                export.FetchResponse(200, listing(["_hidden", "writing:c"], 3)),
                export.FetchResponse(200, listing(["_hidden", "writing:d"], 3)),
            ]
        )
        self.assertEqual(
            result["fullnames"], ["character:a_b", "_hidden", "writing:c", "writing:d"]
        )
        self.assertEqual(
            self.calls, ["/pagelist/p/1", "/pagelist/p/2", "/pagelist/p/3"]
        )
        self.assertEqual(self.delays, [1, 1])
        self.assertEqual(self.run_export([]), result)
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(self.path.parent.stat().st_mode & 0o777, 0o700)
        self.assertEqual(
            set(json.loads(self.path.read_text())),
            {"schema", "source_origin", "completed_page", "highest_page", "fullnames"},
        )

    def test_failed_page_resumes_without_advancing(self):
        with self.assertRaises(export.ListingExportError):
            self.run_export(
                [
                    export.FetchResponse(200, listing(["a"])),
                    export.FetchResponse(403, "secret"),
                ]
            )
        self.assertEqual(json.loads(self.path.read_text())["completed_page"], 1)
        result = self.run_export([export.FetchResponse(200, listing(["b"]))])
        self.assertEqual(
            self.calls, ["/pagelist/p/1", "/pagelist/p/2", "/pagelist/p/2"]
        )
        self.assertEqual(result["fullnames"], ["a", "b"])

    def test_retry_seconds_date_and_network(self):
        result = self.run_export(
            [
                export.FetchResponse(429, "", "5"),
                export.FetchResponse(503, "", "Thu, 01 Jan 1970 00:00:10 GMT"),
                ConnectionError("private transport detail"),
                export.FetchResponse(200, listing(["a"], 1)),
            ],
            now=lambda: 0,
            jitter=lambda: 0.25,
        )
        self.assertEqual(result["completed_page"], 1)
        self.assertEqual(self.delays, [5, 10, 4.25])

    def test_exhaustion_leaves_no_checkpoint(self):
        with self.assertRaises(export.ListingExportError):
            self.run_export([export.FetchResponse(500, "secret")] * 4, jitter=lambda: 0)
        self.assertEqual(len(self.calls), 4)
        self.assertFalse(self.path.exists())

    def test_parser_failure_is_not_retried(self):
        with self.assertRaises(ValueError):
            self.run_export([export.FetchResponse(200, "<html>bad</html>")])
        self.assertEqual(len(self.calls), 1)
        self.assertFalse(self.path.exists())

    def test_origin_and_schema_mismatch_fail_before_fetch(self):
        self.run_export([export.FetchResponse(200, listing(["a"], 1))])
        original = json.loads(self.path.read_text())
        for key, value in [
            ("source_origin", "https://other.test"),
            ("schema", 2),
            ("completed_page", -1),
        ]:
            with self.subTest(key=key):
                self.path.write_text(json.dumps({**original, key: value}))
                with self.assertRaises(export.ListingExportError):
                    self.run_export([])
        self.assertEqual(len(self.calls), 1)

    def test_atomic_replace_failure_preserves_previous_checkpoint(self):
        with self.assertRaises(export.ListingExportError):
            self.run_export(
                [
                    export.FetchResponse(200, listing(["a"])),
                    export.FetchResponse(403, ""),
                ]
            )
        before = self.path.read_bytes()
        with patch.object(export.os, "replace", side_effect=OSError("disk full")):
            with self.assertRaises(OSError):
                self.run_export([export.FetchResponse(200, listing(["b"]))])
        self.assertEqual(self.path.read_bytes(), before)
        self.assertEqual(list(self.path.parent.iterdir()), [self.path])

    def test_unprotected_parent_rejected(self):
        self.path.parent.mkdir(mode=0o755)
        self.path.parent.chmod(0o755)
        with self.assertRaises(export.ListingExportError):
            self.run_export([])
        self.assertEqual(self.calls, [])


if __name__ == "__main__":
    unittest.main()
