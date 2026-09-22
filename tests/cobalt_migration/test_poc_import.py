from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import json
from pathlib import Path
import tarfile
import tempfile
from threading import Thread
import unittest
from urllib.error import HTTPError
from unittest.mock import patch

from tools.cobalt_migration import poc_import as poc


class Store:
    """Behavioral target: persisted pages/files survive importer instances."""

    def __init__(self):
        self.pages = {}
        self.files = {}
        self.pending = {}
        self.next_id = 1
        self.creates = 0
        self.fail_after_page = False
        self.fail_after_file = False

    def rpc(self, method, params):
        if method == "page_get":
            return self.pages.get(params["page"])
        if method == "page_create":
            page = dict(
                params,
                page_id=self.next_id,
                revision_id=1,
                tags=[],
                revision_user_id=params["user_id"],
            )
            self.next_id += 1
            self.pages[params["slug"]] = page
            self.creates += 1
            if self.fail_after_page:
                self.fail_after_page = False
                raise ConnectionError("committed but response lost")
            return page
        if method == "page_edit":
            page = next(
                p for p in self.pages.values() if p["page_id"] == params["page"]
            )
            assert page["revision_id"] == params["last_revision_id"]
            page["tags"] = params["tags"]
            page["revision_id"] += 1
            return {}
        if method == "file_get":
            return self.files.get((params["page_id"], params["file"]))
        if method == "blob_upload":
            token = str(len(self.pending))
            self.pending[token] = None
            return {
                "pending_blob_id": token,
                "presign_url": "http://127.0.0.1/" + token,
            }
        if method == "file_create":
            content = self.pending[params["uploaded_blob_id"]]
            self.files[(params["page_id"], params["name"])] = dict(
                params,
                data=content.hex(),
                size=len(content),
                revision_user_id=params["user_id"],
            )
            self.creates += 1
            if self.fail_after_file:
                self.fail_after_file = False
                raise ConnectionError("file committed but response lost")
            return {}
        raise AssertionError(method)

    def put(self, url, data):
        self.pending[url.rsplit("/", 1)[1]] = data


class ImportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.archive = self.root / "source.tar.gz"
        self.entries = [
            ("source/character_ada.txt", b"name: Ada\n@@: |\n  Hello **wiki**\n"),
            ("source/home_start.txt", b"Welcome\r\n"),
            ("files/character_ada/portrait.png", b"\x89PNG\x00original"),
        ]
        self.make_archive()
        self.listing = {
            "fullnames": ["home:start", "character:ada"],
            "completed_page": 1,
            "highest_page": 1,
        }
        self.metadata = {
            "records": [
                {
                    "fullname": "character:ada",
                    "status": "accepted",
                    "title": "Ada",
                    "tags": ["hero"],
                    "page_id": 73,
                    "revision_number": 9,
                    "updated_at": 12345,
                }
            ]
        }
        self.path = self.root / "plan.json"

    def tearDown(self):
        self.tmp.cleanup()

    def make_archive(self):
        with tarfile.open(self.archive, "w:gz") as archive:
            for name, data in self.entries:
                info = tarfile.TarInfo(name)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))

    def plan(self):
        return poc.prepare_plan(
            self.archive, self.listing, self.metadata, self.path, site_id=10, user_id=20
        )

    def test_cli_resolves_session_before_importing(self):
        self.entries = self.entries[:2]
        self.make_archive()
        self.plan()
        session_file = self.root / "session"
        session_file.write_text("test-session\n")
        session_file.chmod(0o600)
        target = Store()
        session = {"user_id": 20}

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_POST(self):
                request = json.loads(
                    self.rfile.read(int(self.headers["Content-Length"]))
                )
                response = {"jsonrpc": "2.0", "id": request["id"]}
                method, params = request["method"], request["params"]
                if method == "session_get" and params == ["test-session"]:
                    response["result"] = session
                elif method in {"page_get", "page_create", "page_edit"}:
                    response["result"] = target.rpc(method, params)
                else:
                    response["error"] = {"code": -32601, "message": "Method not found"}
                data = json.dumps(response).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

        with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
            thread = Thread(target=server.serve_forever)
            thread.start()
            args = [
                "apply",
                "--archive",
                str(self.archive),
                "--plan",
                str(self.path),
                "--endpoint",
                f"http://127.0.0.1:{server.server_port}/jsonrpc",
                "--session-file",
                str(session_file),
            ]
            try:
                with patch("sys.stdout", new_callable=io.StringIO) as output:
                    poc.main(args)
                self.assertEqual(
                    json.loads(output.getvalue()), {"pages": 2, "attachments": 0}
                )
                self.assertEqual(target.pages["home:start"]["wikitext"], "Welcome\r\n")
                self.assertEqual(target.pages["character:ada"]["tags"], ["hero"])
                for session in ({"user_id": 21}, None):
                    target = Store()
                    with (
                        self.subTest(session=session),
                        self.assertRaisesRegex(
                            poc.PocImportError, "session does not belong"
                        ),
                    ):
                        poc.main(args)
                    self.assertEqual(target.creates, 0)
            finally:
                server.shutdown()
                thread.join()

    def test_full_plan_and_exact_content_idempotence(self):
        plan = self.plan()
        self.assertEqual((len(plan["pages"]), len(plan["attachments"])), (2, 1))
        self.assertEqual(plan["pages"][0]["metadata"]["revision_number"], 9)
        self.assertEqual(plan["pages"][1]["metadata_status"], "not_acquired")
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)
        target = Store()
        first = poc.apply_plan(self.archive, self.path, target.rpc, target.put)
        second = poc.apply_plan(self.archive, self.path, target.rpc, target.put)
        self.assertEqual(first, second)
        self.assertEqual(first, {"pages": 2, "attachments": 1})
        self.assertEqual(target.creates, 3)
        self.assertEqual(
            target.pages["home:start"]["wikitext"].encode(), b"Welcome\r\n"
        )
        self.assertEqual(target.pages["character:ada"]["tags"], ["hero"])
        self.assertEqual(
            bytes.fromhex(next(iter(target.files.values()))["data"]), self.entries[2][1]
        )

    def test_technical_admin_id_retained_in_plan_and_request_attribution(self):
        plan = poc.prepare_plan(
            self.archive, self.listing, self.metadata, self.path, site_id=10, user_id=-1
        )
        self.assertEqual(plan["user_id"], -1)
        self.assertEqual(json.loads(self.path.read_text())["user_id"], -1)
        target = Store()
        requests = []

        def rpc(method, params):
            requests.append((method, params.copy()))
            return target.rpc(method, params)

        self.assertEqual(
            poc.apply_plan(self.archive, self.path, rpc, target.put),
            {"pages": 2, "attachments": 1},
        )
        writes = [
            params
            for method, params in requests
            if method in {"page_create", "page_edit", "file_create"}
        ]
        self.assertEqual(len(writes), 4)
        self.assertTrue(all(params["user_id"] == -1 for params in writes))
        self.assertTrue(
            all(page["revision_user_id"] == -1 for page in target.pages.values())
        )
        self.assertEqual(next(iter(target.files.values()))["revision_user_id"], -1)

    def test_invalid_target_ids_rejected_before_plan_is_written(self):
        cases = [
            (site_id, 20) for site_id in (True, False, 0, -1, -2, 1.0, "1", None)
        ] + [(10, user_id) for user_id in (True, False, 0, -2, -100, 1.0, "-1", None)]
        for site_id, user_id in cases:
            with self.subTest(site_id=site_id, user_id=user_id):
                with self.assertRaises(poc.PocImportError):
                    poc.prepare_plan(
                        self.archive,
                        self.listing,
                        self.metadata,
                        self.path,
                        site_id=site_id,
                        user_id=user_id,
                    )
                self.assertFalse(self.path.exists())

    def test_restart_after_commit_before_response_reconciles(self):
        self.plan()
        target = Store()
        target.fail_after_page = True
        with self.assertRaises(ConnectionError):
            poc.apply_plan(self.archive, self.path, target.rpc, target.put)
        self.assertEqual(target.creates, 1)
        result = poc.apply_plan(self.archive, self.path, target.rpc, target.put)
        self.assertEqual(result["pages"], 2)
        self.assertEqual(target.creates, 3)

    def test_restart_after_file_commit_keeps_single_attachment(self):
        self.plan()
        target = Store()
        target.fail_after_file = True
        with self.assertRaises(ConnectionError):
            poc.apply_plan(self.archive, self.path, target.rpc, target.put)
        result = poc.apply_plan(self.archive, self.path, target.rpc, target.put)
        self.assertEqual(result, {"pages": 2, "attachments": 1})
        self.assertEqual(target.creates, 3)
        self.assertEqual(len(target.pending), 1)

    def test_existing_plan_cannot_be_replaced_with_new_metadata(self):
        self.plan()
        before = self.path.read_bytes()
        self.metadata["records"][0]["title"] = "Changed"
        with self.assertRaises(poc.PocImportError):
            self.plan()
        self.assertEqual(self.path.read_bytes(), before)

    def test_foreign_existing_page_refused_before_any_writes(self):
        self.plan()
        target = Store()
        target.pages["home:start"] = {
            "revision_comments": "human",
            "revision_user_id": 20,
        }
        with self.assertRaises(poc.PocImportError):
            poc.apply_plan(self.archive, self.path, target.rpc, target.put)
        self.assertEqual(target.creates, 0)

    def test_changed_imported_source_refused(self):
        self.plan()
        target = Store()
        poc.apply_plan(self.archive, self.path, target.rpc, target.put)
        target.pages["character:ada"]["wikitext"] = "human edit"
        with self.assertRaises(poc.PocImportError):
            poc.apply_plan(self.archive, self.path, target.rpc, target.put)
        self.assertEqual(target.creates, 3)

    def test_archive_change_refused_without_target_calls(self):
        self.plan()
        self.entries[0] = (self.entries[0][0], b"changed")
        self.make_archive()
        with self.assertRaises(poc.PocImportError):
            poc.apply_plan(
                self.archive, self.path, lambda *_: self.fail("target called"), None
            )

    def test_plan_collision_missing_extra_and_orphan_attachments(self):
        for names in [
            ["a:b", "a_b"],
            ["character:ada"],
            ["home:start", "character:ada", "extra"],
        ]:
            with self.subTest(names=names):
                self.listing["fullnames"] = names
                with self.assertRaises(poc.PocImportError):
                    self.plan()
        self.listing["fullnames"] = ["home:start", "character:ada"]
        self.entries.append(("files/unknown/a.txt", b"x"))
        self.make_archive()
        with self.assertRaises(poc.PocImportError):
            self.plan()

    def test_duplicate_archive_members_rejected(self):
        self.entries.append(self.entries[0])
        self.make_archive()
        with self.assertRaises(ValueError):
            self.plan()

    def test_changed_attachment_and_foreign_attachment_refused(self):
        self.plan()
        target = Store()
        poc.apply_plan(self.archive, self.path, target.rpc, target.put)
        file = next(iter(target.files.values()))
        file["data"] = b"changed".hex()
        with self.assertRaises(poc.PocImportError):
            poc.apply_plan(self.archive, self.path, target.rpc, target.put)
        self.assertEqual(target.creates, 3)


class TransportTests(unittest.TestCase):
    def test_transient_reads_retry_but_writes_never_repeat(self):
        class Opener:
            def __init__(self):
                self.calls = 0

            def open(self, request, timeout):
                self.calls += 1
                if self.calls == 1:
                    raise HTTPError(
                        request.full_url, 503, "busy", {"Retry-After": "3"}, None
                    )
                return io.BytesIO(b'{"jsonrpc":"2.0","id":1,"result":null}')

        client = poc.LoopbackRpc("http://127.0.0.1:2747/jsonrpc", "private", 10)
        for method, params in (
            ("page_get", {"page": "home:start"}),
            ("session_get", ["private"]),
        ):
            client.opener = Opener()
            with self.subTest(method=method), patch.object(poc.time, "sleep") as sleep:
                self.assertIsNone(client.rpc(method, params))
                self.assertEqual(sleep.call_args.args, (3.0,))
        client.opener = Opener()
        with self.assertRaises(poc.PocImportError):
            client.rpc("page_create", {"slug": "home:start"})
        self.assertEqual(client.opener.calls, 1)

    def test_transport_refuses_non_loopback_and_redirect_endpoints(self):
        for url in [
            "https://example.org/jsonrpc",
            "http://localhost/jsonrpc",
            "http://127.0.0.1@evil.test/jsonrpc",
        ]:
            with self.subTest(url=url), self.assertRaises(poc.PocImportError):
                poc.LoopbackRpc(url, "private", 10)


if __name__ == "__main__":
    unittest.main()
