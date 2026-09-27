"""Read-only CLI preview across source, RPC, file bytes, and output artifacts."""

import contextlib
import hashlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tests.cobalt_migration.test_wikidot_sync import FakeReplica, files_listing
from tools.cobalt_migration import wikidot_sync as sync
from tools.cobalt_migration.wikidot_files import format_size


class ReadOnlyReplica:
    def __init__(self, pages, files):
        self.pages = pages
        self.files = files
        self.reads = []

    def rpc(self, method, params):
        self.reads.append(method)
        if method == "session_get":
            return {"user_id": -1}
        if method == "page_get":
            return self.pages.get(params["page"])
        if method == "page_get_files":
            return [metadata for metadata, _ in self.files.values()]
        if method == "file_get":
            metadata, data = self.files[params["file"]]
            return {**metadata, "data": data.hex()}
        raise AssertionError(f"mutation attempted: {method}")

    def put(self, *args):
        raise AssertionError("upload attempted")


def page(slug, owner=-1):
    return {
        "page_id": 17 if slug == "story" else 18,
        "revision_id": 5,
        "revision_user_id": owner,
        "revision_comments": "Wikidot sync (rev. 3)",
        "slug": slug,
        "wikitext": "old body",
        "title": "Old title",
        "tags": [],
    }


def file(name, data):
    return (
        {
            "name": name,
            "file_id": len(name),
            "revision_id": 4,
            "revision_user_id": -1,
            "revision_comments": "Wikidot sync (file)",
            "s3_hash": hashlib.sha512(data).hexdigest(),
            "size": len(data),
        },
        data,
    )


def row(slug, revision, flags="S", comments=""):
    return {
        "slug": slug,
        "title": slug,
        "revision": revision,
        "flags": flags,
        "changed_at": revision,
        "comments": comments,
        "user_slug": "u",
        "user_name": "U",
        "user_id": 1,
    }


class PreviewTest(unittest.TestCase):
    def invoke(self, changes, pages, files, source_pages, source_files):
        replica = ReadOnlyReplica(pages, files)
        login = mock.Mock()
        login.rpc.return_value = {"session_token": "secret-token"}
        source_urls = []

        def source_request(url):
            if url.endswith(("/story", "/human", "/new")):
                slug = url.rsplit("/", 1)[1]
                return (
                    200,
                    f"WIKIREQUEST.info.pageId = {source_pages[slug]['id']};".encode(),
                )
            name = url.rsplit("/", 1)[1]
            source_urls.append(name)
            return 200, source_files[name]

        def source_module(origin, **params):
            if params["moduleName"] == "files/PageFilesModule":
                rows = "".join(
                    f'<tr id="file-row-{i}"><td><a href="/local--files/story/{name}">{name}</a></td>'
                    f"<td>x</td><td>{format_size(len(data))}</td></tr>"
                    for i, (name, data) in enumerate(source_files.items(), 1)
                )
                return {
                    "status": "ok",
                    "body": f"<p>Total files: {len(source_files)}</p>{rows}",
                }
            slug = (
                params["fullname"]
                if "fullname" in params
                else next(
                    s
                    for s, state in source_pages.items()
                    if str(state["id"]) == str(params["page_id"])
                )
            )
            state = source_pages[slug]
            if params["moduleName"] == "viewsource/ViewSourceModule":
                return {
                    "status": "ok",
                    "body": f'<div class="page-source">{state["body"]}</div>',
                }
            return {
                "status": "ok",
                "body": '<span class="sync-meta">New title|||'
                '<span class="odate time_1">x</span>|<span class="odate time_2">x</span></span>',
            }

        with tempfile.TemporaryDirectory() as tmp:
            password = Path(tmp) / "password"
            password.write_text("password")
            with (
                mock.patch.object(sync, "LoopbackRpc", side_effect=[login, replica]),
                mock.patch.object(sync, "paced", return_value=changes),
                mock.patch.object(sync, "module", side_effect=source_module),
                mock.patch.object(sync, "wikidot_request", side_effect=source_request),
                contextlib.redirect_stdout(io.StringIO()) as output,
            ):
                args = [
                    "https://source.wikidot.com",
                    "https://target",
                    "http://127.0.0.1:1/jsonrpc",
                    "6000000",
                    str(password),
                    "0",
                    tmp,
                ]
                try:
                    sync.main(["--dry-run", *args])
                    failure = None
                except RuntimeError as error:
                    failure = str(error)
            report = json.loads((Path(tmp) / "sync-report.json").read_text())
            sql = (Path(tmp) / "sync-apply.sql").read_text()
        return replica, report, sql, output.getvalue(), source_urls, failure

    def test_previews_pages_files_and_sql_without_target_writes(self):
        old = b"old" * 500
        same = b"same" * 500
        source_files = {"added": b"new", "replaced": b"new" * 500, "same": same}
        pages = {"story": page("story"), "human": page("human", 42)}
        files = {
            name: file(name, data)
            for name, data in {
                "replaced": old,
                "same": same,
                "deleted": b"remove",
                "untracked": b"keep",
            }.items()
        }
        changes = [
            row("story", 13),
            row("story", 12, "F", 'Uploaded file "added".'),
            row("story", 11, "F", 'Uploaded file "replaced".'),
            row("story", 10, "F", 'Uploaded file "same".'),
            row("story", 9, "F", 'File "deleted" deleted'),
            row("human", 8),
            row("new", 7),
            row("new", 6, "F", 'Uploaded file "added".'),
        ]
        source_pages = {
            slug: {"id": i, "body": "fresh body"}
            for i, slug in enumerate(("story", "human", "new"), 1)
        }
        replica, report, sql, stdout, downloads, failure = self.invoke(
            changes, pages, files, source_pages, source_files
        )
        self.assertIsNone(failure)
        self.assertEqual(
            report["pages"],
            {
                "story": "would-edit title,wikitext",
                "human": "kept: current page revision is not import-owned",
                "new": "would-create",
            },
        )
        self.assertEqual(
            report["files"]["story"]["files"],
            {
                "added": "would-create",
                "replaced": "would-replace",
                "same": "unchanged",
                "deleted": "would-delete",
                "untracked": "kept: absent on Wikidot, but no revision in the window removed it",
            },
        )
        self.assertEqual(
            report["files"]["new"]["files"],
            {
                "added": "would-create",
                "replaced": "would-create",
                "same": "would-create",
            },
        )
        self.assertEqual(
            sorted(downloads),
            ["added", "added", "replaced", "replaced", "same", "same"],
        )
        self.assertIn("UPDATE page SET", sql)
        self.assertIn("INSERT INTO wikidot_site_change", sql)
        self.assertNotIn("WHERE site_id = 6000000 AND slug = 'human'", sql)
        self.assertEqual(replica.pages, pages)
        self.assertEqual(replica.files, files)
        self.assertNotIn("fresh body", stdout)
        self.assertNotIn("secret-token", stdout)
        self.assertNotIn("https://", stdout)
        self.assertIn("would-create", stdout)

    def test_file_preview_matches_apply_using_stored_hash_without_mutating(self):
        source = {"same": b"same", "changed": b"new", "added": b"add"}
        events = {
            "touched": {"same", "changed", "added"},
            "gone": {"removed"},
            "unrecognized": [],
        }
        existing = {
            "same": FakeReplica.file("same", b"same", 1),
            "changed": FakeReplica.file("changed", b"old", 2),
            "removed": FakeReplica.file("removed", b"gone", 3),
        }
        preview = FakeReplica(
            ["story"],
            files={1: {name: metadata.copy() for name, metadata in existing.items()}},
        )
        applied = FakeReplica(
            ["story"],
            files={1: {name: metadata.copy() for name, metadata in existing.items()}},
        )
        listing = {"status": "ok", "body": files_listing(source)}

        def request(url):
            return 200, source[url.rsplit("/", 1)[1]]

        with (
            mock.patch.object(sync, "module", return_value=listing),
            mock.patch.object(sync, "wikidot_request", side_effect=request),
        ):
            planned = sync.sync_files(
                preview,
                6000000,
                -1,
                "https://source.wikidot.com",
                1,
                1,
                events,
                dry_run=True,
            )
            actual = sync.sync_files(
                applied, 6000000, -1, "https://source.wikidot.com", 1, 1, events
            )

        self.assertEqual(
            planned,
            {
                "added": "would-create",
                "changed": "would-replace",
                "removed": "would-delete",
                "same": "unchanged",
            },
        )
        self.assertEqual(
            actual,
            {
                "added": "created",
                "changed": "updated",
                "removed": "deleted",
                "same": "unchanged",
            },
        )
        self.assertEqual(preview.files[1], existing)
        self.assertEqual(applied.files[1]["same"], existing["same"])
        self.assertEqual(
            applied.files[1]["changed"]["s3_hash"],
            hashlib.sha512(source["changed"]).hexdigest(),
        )
        self.assertEqual(
            applied.files[1]["added"]["s3_hash"],
            hashlib.sha512(source["added"]).hexdigest(),
        )
        self.assertNotIn("removed", applied.files[1])

    def test_actionable_rename_blocks_dependent_planning_and_sql(self):
        changes = [
            row("new", 13, "R", 'You successfully renamed the page: "old" to "new".')
        ]
        replica, report, sql, _, downloads, failure = self.invoke(
            changes, {"old": page("old")}, {}, {}, {}
        )
        self.assertIn("rename", failure)
        self.assertEqual(report["renames"][0]["outcome"], "would-move")
        self.assertIn("blocker", report)
        self.assertEqual(report["pages"], {})
        self.assertNotIn("UPDATE page SET", sql)
        self.assertNotIn("INSERT INTO wikidot_site_change", sql)
        self.assertEqual(downloads, [])
        self.assertNotIn("page_import", replica.reads)


if __name__ == "__main__":
    unittest.main()
