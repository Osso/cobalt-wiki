import json
import tempfile
import unittest
from pathlib import Path

from tools.cobalt_migration.history_import import page_history


def write_page(directory, bodies):
    directory.mkdir()
    revisions = [
        {"number": 1, "revision_id": 11, "author_id": 7570574, "created_at": 1788744406,
         "comments": None, "flags": [{"marker": "S", "description": "content source text changed"}]},
        {"number": 0, "revision_id": 10, "author_id": 7570574, "created_at": 1788700000,
         "comments": "Created", "flags": [{"marker": "N", "description": "new page created"}]},
    ]
    (directory / "checkpoint.json").write_text(json.dumps({
        "schema": 1, "revisions": revisions, "bodies": bodies,
    }))
    (directory / "revision-11.html").write_text('<div class="page-source">\nnew\n</div>')
    (directory / "revision-10.html").write_text("")


class HistoryImportTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)

    def test_bodies_become_revisions_and_refusals_stay_gaps(self):
        page = Path(self.temp.name) / "1469283886"
        write_page(page, {
            "11": {"status": 200, "acquired_at": 1790195638.5, "wikitext": "new",
                   "representation": "display-decoded-not-byte-exact"},
            "10": {"status": 200, "acquired_at": 1790195639, "module_status": "no_permission"},
        })
        revisions, gaps = page_history(page)
        self.assertEqual(gaps, [0])
        self.assertEqual(revisions, [{
            "source_revision_id": 11, "source_revision_number": 1,
            "source_author_id": 7570574, "source_created_at": "2026-09-07T01:26:46Z",
            "source_comments": "", "source_flags": ["S"],
            "source_title": None, "source_slug": None, "source_tags": None,
            "wikitext": "new", "raw_source_html": '<div class="page-source">\nnew\n</div>',
            "acquired_at": "2026-09-23T20:33:58.500000Z",
            "representation": "display-decoded-not-byte-exact",
        }])

    def test_incomplete_acquisition_is_skipped(self):
        page = Path(self.temp.name) / "1"
        write_page(page, {"11": {"status": 200, "acquired_at": 1, "wikitext": "new",
                                 "representation": "display-decoded-not-byte-exact"}})
        self.assertIsNone(page_history(page))


if __name__ == "__main__":
    unittest.main()
