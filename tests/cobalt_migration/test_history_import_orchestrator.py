"""CLI history import regression at the loopback HTTP and process boundary."""

import copy
import json
import socket
import stat
import subprocess
import sys
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from tools.cobalt_migration.history_acquire import _inventory
from tools.cobalt_migration.history_export import HistoryResponse, export_history
from tools.cobalt_migration.poc_import import _digest


ROOT = Path(__file__).resolve().parents[2]
SITE_ID = 6000000
SOURCE_ORIGIN = "https://history.example.test"
WIRE_CAP = 10 * 1024 * 1024


def protected_json(path, value):
    path.write_text(json.dumps(value), encoding="utf-8")
    path.chmod(0o600)


def revision_row(number, revision_id):
    comment = "" if number == 0 else "Saved"
    marker = "N" if number == 0 else "S"
    return (
        f'<tr id="revision-row-{revision_id}"><td>{number}.</td><td></td>'
        f'<td><span class="spantip" title="source">{marker}</span></td>'
        f'<td><a onclick="showSource({revision_id})">Source</a></td>'
        '<td><span class="printuser"><a onclick="userInfo(7570574)">Ada</a></span></td>'
        f'<td><span class="odate time_1600000000">date</span><td>{comment}</td></td></tr>'
    )


class PersistentTarget:
    """In-process durable target; each CLI invocation is a new OS process."""

    def __init__(self):
        self.pages = {}
        self.history = {}
        self.import_requests = []
        self.wire_bytes = []
        self.lost_response_once = False
        self.lock = threading.Lock()

    def add_page(self, source_id, slug):
        page = {
            "page_id": source_id + 10000,
            "slug": slug,
            "revision_id": source_id + 20000,
            "wikitext": "Unchanged current source\n",
            "page_revision_count": 2,
        }
        self.pages[slug] = page
        self.history[page["page_id"]] = {}
        return page

    def handle(self, method, params):
        if method == "login":
            assert params["password"] == "fixture-only-password"
            return {"session_token": "fixture-session"}
        assert method in {
            "page_get",
            "page_imported_history",
            "page_imported_revision",
            "import_wikidot_history",
        }, f"unexpected native/source mutation: {method}"
        if method == "page_get":
            return self.pages.get(params["page"])
        rows = self.history[params["page_id"]]
        if method == "page_imported_history":
            before = params.get("before_revision")
            numbers = sorted(
                (n for n in rows if before is None or n < before), reverse=True
            )[: params["limit"]]
            return [self.summary(rows[n]) for n in numbers]
        if method == "page_imported_revision":
            row = rows.get(params["source_revision_number"])
            return (
                None
                if row is None
                else {"metadata": self.summary(row), "wikitext": row["wikitext"]}
            )
        assert params["expected_revision_id"] == next(
            p["revision_id"]
            for p in self.pages.values()
            if p["page_id"] == params["page_id"]
        )
        self.import_requests.append(params)
        inserted = 0
        for row in params["revisions"]:
            number = row["source_revision_number"]
            if number in rows:
                if rows[number] != {**row, "source_page_id": params["source_page_id"]}:
                    raise ValueError("conflicting imported history")
            else:
                rows[number] = {**row, "source_page_id": params["source_page_id"]}
                inserted += 1
        return {"inserted": inserted}

    @staticmethod
    def summary(row):
        fields = (
            "source_page_id",
            "source_revision_id",
            "source_revision_number",
            "source_author_id",
            "source_created_at",
            "source_comments",
            "source_flags",
            "source_title",
            "source_slug",
            "source_tags",
            "representation",
        )
        return {field: row[field] for field in fields}


class HistoryImportOrchestratorTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.archive = self.root / "history"
        self.archive.mkdir(mode=0o700)
        self.plan_path = self.root / "plan.json"
        self.password_path = self.root / "password"
        self.password_path.write_text("fixture-only-password\n")
        self.password_path.chmod(0o600)
        self.report_path = self.root / "report.json"
        self.target = PersistentTarget()
        target = self.target

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_POST(self):
                size = int(self.headers["Content-Length"])
                data = self.rfile.read(size)
                request = json.loads(data)
                method = request["method"]
                with target.lock:
                    if method == "import_wikidot_history":
                        target.wire_bytes.append(size)
                    try:
                        if size > WIRE_CAP:
                            raise ValueError("history request exceeds wire cap")
                        result = target.handle(method, request["params"])
                    except (
                        AssertionError,
                        KeyError,
                        StopIteration,
                        ValueError,
                    ) as error:
                        reply = {
                            "jsonrpc": "2.0",
                            "id": request["id"],
                            "error": {"code": -32000, "message": str(error)},
                        }
                    else:
                        if (
                            method == "import_wikidot_history"
                            and target.lost_response_once
                        ):
                            target.lost_response_once = False
                            self.connection.shutdown(socket.SHUT_RDWR)
                            self.connection.close()
                            return
                        reply = {
                            "jsonrpc": "2.0",
                            "id": request["id"],
                            "result": result,
                        }
                payload = json.dumps(reply).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.addCleanup(self.thread.join)
        self.addCleanup(self.server.server_close)
        self.addCleanup(self.server.shutdown)

    def archive_pages(self, definitions):
        """Create actual hashed, owner-only exporter checkpoints from fake responses."""
        pages = []
        for source_id, slug, bodies in definitions:
            self.target.add_page(source_id, slug)
            pages.append(
                {
                    "fullname": slug,
                    "metadata_status": "accepted",
                    "metadata": {"page_id": source_id},
                }
            )
            rows = [
                revision_row(number, source_id * 100 + number)
                for number in sorted(bodies, reverse=True)
            ]
            list_html = (
                '<div class="page-history"><table>' + "".join(rows) + "</table></div>"
            )

            def fetch(request):
                if request["moduleName"] == "history/PageRevisionListModule":
                    return HistoryResponse(200, "raw-list", list_html)
                number = request["revision_id"] - source_id * 100
                html = f'<div class="page-source">\n{bodies[number]}\n</div>'
                return HistoryResponse(200, "raw-source", html)

            export_history(
                SOURCE_ORIGIN,
                source_id,
                self.archive / str(source_id),
                fetch,
                sleep=lambda _seconds: None,
                jitter=lambda: 0,
                now=lambda: 1780000000,
            )
        plan = {"schema": 1, "site_id": SITE_ID, "pages": pages}
        plan["plan_sha256"] = _digest(plan)
        protected_json(self.plan_path, plan)
        identities, unresolved, digest = _inventory(plan)
        self.assertEqual((identities, unresolved), ([d[0] for d in definitions], []))
        protected_json(
            self.archive / "site-progress.json",
            {
                "schema": 1,
                "source_origin": SOURCE_ORIGIN,
                "inventory_sha256": digest,
                "unresolved_metadata": [],
                "pages": {
                    str(source_id): {
                        "listed": len(bodies),
                        "bodies": len(bodies),
                        "unavailable_bodies": 0,
                        "unobserved_ranges": [],
                    }
                    for source_id, _slug, bodies in definitions
                },
                "failed_page_id": None,
                "listed_revisions": sum(len(bodies) for _, _, bodies in definitions),
                "acquired_bodies": sum(len(bodies) for _, _, bodies in definitions),
            },
        )
        for file in self.archive.rglob("*"):
            if file.is_file():
                self.assertEqual(stat.S_IMODE(file.stat().st_mode), 0o600)

    def cli(self):
        return subprocess.run(
            [
                sys.executable,
                "-m",
                "tools.cobalt_migration.history_import",
                str(self.plan_path),
                str(self.archive),
                f"http://127.0.0.1:{self.server.server_port}/jsonrpc",
                str(SITE_ID),
                str(self.password_path),
                str(self.report_path),
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=40,
            check=False,
        )

    def assert_native_unchanged(self, before):
        self.assertEqual(self.target.pages, before)

    def test_large_page_is_split_under_wire_cap_and_preserves_every_record(self):
        bodies = {n: chr(65 + n) * (2 * 1024 * 1024 + 100) for n in (2, 1, 0)}
        self.archive_pages([(101, "home:start", bodies)])
        before = json.loads(json.dumps(self.target.pages))
        result = self.cli()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertGreater(len(self.target.wire_bytes), 1)
        self.assertTrue(all(size <= WIRE_CAP for size in self.target.wire_bytes))
        rows = self.target.history[10101]
        self.assertEqual(set(rows), {0, 1, 2})
        for number, body in bodies.items():
            row = rows[number]
            self.assertEqual(row["source_revision_id"], 10100 + number)
            self.assertEqual(row["wikitext"], body)
            self.assertEqual(
                row["raw_source_html"], f'<div class="page-source">\n{body}\n</div>'
            )
            self.assertEqual(row["source_flags"], ["N" if number == 0 else "S"])
            self.assertEqual(row["source_comments"], "" if number == 0 else "Saved")
            self.assertEqual(row["source_created_at"], "2020-09-13T12:26:40Z")
            self.assertEqual(row["source_author_id"], 7570574)
            self.assertEqual(row["acquired_at"], "2026-05-28T20:26:40Z")
            self.assertEqual(row["representation"], "display-decoded-not-byte-exact")
            self.assertIsNone(row["source_title"])
            self.assertIsNone(row["source_slug"])
            self.assertIsNone(row["source_tags"])
        self.assert_native_unchanged(before)

    def test_matching_existing_is_skipped_and_later_conflict_aborts_preflight(self):
        self.archive_pages(
            [
                (101, "home:start", {1: "missing before conflict", 0: "original"}),
                (102, "home:other", {0: "target conflict"}),
            ]
        )
        first = self.target.history[10101]
        first[0] = {
            "source_page_id": 101,
            "source_revision_id": 10100,
            "source_revision_number": 0,
            "source_author_id": 7570574,
            "source_created_at": "2020-09-13T12:26:40Z",
            "source_comments": "",
            "source_flags": ["N"],
            "source_title": None,
            "source_slug": None,
            "source_tags": None,
            "wikitext": "original",
            "raw_source_html": '<div class="page-source">\noriginal\n</div>',
            "acquired_at": "2026-05-28T20:26:40Z",
            "representation": "display-decoded-not-byte-exact",
        }
        second = self.target.history[10102]
        second[0] = {
            **first[0],
            "source_page_id": 102,
            "source_revision_id": 10200,
            "wikitext": "different target body",
        }
        before_history = copy.deepcopy(self.target.history)
        before_pages = json.loads(json.dumps(self.target.pages))
        result = self.cli()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.target.import_requests, [])
        self.assertEqual(self.target.history, before_history)
        self.assert_native_unchanged(before_pages)

    def test_target_guard_drift_on_resume_preserves_current_and_history(self):
        self.archive_pages([(101, "home:start", {0: "original"})])
        first = self.cli()
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertTrue(self.report_path.exists())
        self.target.pages["home:start"]["revision_id"] += 1
        before_pages = json.loads(json.dumps(self.target.pages))
        before_history = copy.deepcopy(self.target.history)
        writes = len(self.target.import_requests)
        result = self.cli()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(self.target.import_requests), writes)
        self.assertEqual(self.target.history, before_history)
        self.assert_native_unchanged(before_pages)

    def test_lost_response_after_commit_resumes_in_fresh_process_without_replay(self):
        bodies = {n: chr(65 + n) * (2 * 1024 * 1024 + 100) for n in (2, 1, 0)}
        self.archive_pages([(101, "home:start", bodies)])
        before = json.loads(json.dumps(self.target.pages))
        self.target.lost_response_once = True
        first = self.cli()
        self.assertNotEqual(first.returncode, 0)
        committed = {
            row["source_revision_id"]
            for request in self.target.import_requests
            for row in request["revisions"]
        }
        self.assertTrue(committed)
        self.assertLess(len(committed), 3)
        self.assertTrue(self.report_path.exists())
        request_count = len(self.target.import_requests)
        second = self.cli()
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertTrue(all(size <= WIRE_CAP for size in self.target.wire_bytes))
        replay = {
            row["source_revision_id"]
            for request in self.target.import_requests[request_count:]
            for row in request["revisions"]
        }
        self.assertFalse(replay & committed)
        self.assertEqual(set(self.target.history[10101]), {0, 1, 2})
        self.assert_native_unchanged(before)


if __name__ == "__main__":
    unittest.main()
