import contextlib
import hashlib
import io
import json
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

from tools.cobalt_migration import wikidot_sync
from tools.cobalt_migration.wikidot_files import format_size
from tools.cobalt_migration.wikidot_sync import (
    apply_sql,
    decode_view_source,
    parse_meta,
    parse_renames,
    plan_rename,
    sync_files,
    sync_page,
    sync_renames,
)

# Shape of cobalt-company.wikidot.com ViewSourceModule output for "roster".
VIEW_SOURCE = """<h1>Page source</h1>

<div class="page-source">
	[[include <a href="/toc">toc</a>]]<br />
<br />
[[div style=&quot;width:95%;&quot;]]<br />
Gray eyes.&nbsp;&nbsp;His face is pleasant.<br />
[[/div]]
</div>"""

DATE = "format_%25e%20%25b%20%25Y%2C%20%25H%3A%25M%7Cagohover"
LISTING = (
    '<div class="list-pages-box"><p><span class="sync-meta">(2026-09-19) A Bold Idea'
    "|alli atley rated-t|_completed|"
    f'<span class="odate time_1790088140 {DATE}">22 Sep 2026 14:42</span>|'
    f'<span class="odate time_1790088156 {DATE}">22 Sep 2026 14:42</span></span></p></div>'
)


class WikidotSyncTest(unittest.TestCase):
    def test_retry_log_does_not_disclose_request_url_or_failure_reason(self):
        url = "https://source.wikidot.com/page?token=private-query"
        failure = urllib.error.HTTPError(url, 503, "private-reason", {}, None)
        stderr = io.StringIO()
        with (
            mock.patch.object(wikidot_sync, "throttle"),
            mock.patch.object(wikidot_sync.time, "sleep"),
            mock.patch.object(
                wikidot_sync.urllib.request, "urlopen", side_effect=failure
            ),
            contextlib.redirect_stderr(stderr),
            self.assertRaises(urllib.error.HTTPError),
        ):
            wikidot_sync.wikidot_request(url)
        self.assertEqual(
            stderr.getvalue().splitlines(),
            [f"retry {attempt}/4: HTTPError (http 503)" for attempt in range(1, 4)],
        )

    def test_view_source_decodes_to_the_saved_source(self):
        self.assertEqual(
            decode_view_source(VIEW_SOURCE),
            '[[include toc]]\n\n[[div style="width:95%;"]]\nGray eyes.  His face is pleasant.\n[[/div]]',
        )

    def test_listing_gives_title_all_tags_and_dates(self):
        self.assertEqual(
            parse_meta(LISTING),
            {
                "title": "(2026-09-19) A Bold Idea",
                "tags": ["_completed", "alli", "atley", "rated-t"],
                "created_at": 1790088140,
                "updated_at": 1790088156,
            },
        )

    def test_apply_sql_sets_dates_and_adds_revisions(self):
        sql = apply_sql(
            6000000,
            {"writing:it's": {"created_at": 1, "updated_at": 2}},
            [
                {
                    "slug": "writing:it's",
                    "title": "It's",
                    "revision": 0,
                    "flags": "N",
                    "changed_at": 1,
                    "user_slug": "allicat",
                    "user_name": "Allicat",
                    "user_id": 7570574,
                    "comments": "",
                }
            ],
        )
        self.assertIn(
            "UPDATE page SET created_at = to_timestamp(1), updated_at = to_timestamp(2) "
            "WHERE site_id = 6000000 AND slug = 'writing:it''s' AND deleted_at IS NULL;",
            sql,
        )
        self.assertIn(
            "VALUES (6000000, 'writing:it''s', 'It''s', 0, 'N', to_timestamp(1), 'allicat', "
            "'Allicat', 7570574, '') ON CONFLICT DO NOTHING;",
            sql,
        )

    def test_apply_sql_moves_rows_to_renamed_pages(self):
        sql = apply_sql(
            6000000,
            {},
            [],
            renames=[
                {
                    "from": "character:brynnal",
                    "to": "character:baird",
                    "outcome": "moved",
                },
                {"from": "a", "to": "b", "outcome": "already moved"},
                {
                    "from": "c",
                    "to": "d",
                    "outcome": "skip: both slugs exist on the replica",
                },
                {"from": "e", "to": "f", "outcome": wikidot_sync.NOTHING_TO_MOVE},
            ],
        )
        self.assertIn(
            "UPDATE wikidot_site_change SET page_slug = 'character:baird' "
            "WHERE site_id = 6000000 AND page_slug = 'character:brynnal';",
            sql,
        )
        self.assertIn(
            "SET page_slug = 'b' WHERE site_id = 6000000 AND page_slug = 'a';", sql
        )
        self.assertNotIn("page_slug = 'c'", sql)
        self.assertIn(
            "SET page_slug = 'f' WHERE site_id = 6000000 AND page_slug = 'e';", sql
        )


def change(slug, flags, changed_at, revision, comments):
    return {
        "slug": slug,
        "flags": flags,
        "changed_at": changed_at,
        "revision": revision,
        "comments": comments,
    }


# Rows as SiteChanges lists them (newest first, under the page's current slug).
RENAME_ROWS = [
    change(
        "writing:2026-09-18-a-collection-of-thought-exercises",
        "R",
        1789804618,
        4,
        'You successfully renamed the page: "writing:2028-09-18-a-collection-of-thought-exercises" '
        'to "writing:2026-09-18-a-collection-of-thought-exercises".',
    ),
    change("character:baird", "S", 1789090700, 119, ""),
    change(
        "character:baird",
        "R",
        1789090609,
        118,
        'You successfully renamed the page: "character:brynnal" to "character:baird".',
    ),
]


class RenameTest(unittest.TestCase):
    def test_reads_old_and_new_names_oldest_first(self):
        self.assertEqual(
            [(r["from"], r["to"], r["revision"]) for r in parse_renames(RENAME_ROWS)],
            [
                ("character:brynnal", "character:baird", 118),
                (
                    "writing:2028-09-18-a-collection-of-thought-exercises",
                    "writing:2026-09-18-a-collection-of-thought-exercises",
                    4,
                ),
            ],
        )

    def test_unrecognized_rename_comment_has_no_names(self):
        [rename] = parse_renames([change("x", "R", 1, 2, "Renamed by magic")])
        self.assertEqual((rename["from"], rename["to"]), (None, None))
        self.assertEqual(
            plan_rename(rename, True, False), "skip: rename comment not recognized"
        )

    def test_decision_by_replica_state(self):
        rename = {"from": "character:brynnal", "to": "character:baird"}
        self.assertEqual(plan_rename(rename, True, False), "move")
        self.assertEqual(plan_rename(rename, False, True), "already moved")
        self.assertEqual(
            plan_rename(rename, True, True), "skip: both slugs exist on the replica"
        )
        self.assertTrue(plan_rename(rename, False, False).startswith("nothing to move"))


POC_MARKER = f"Cobalt POC import {'a' * 64}; source authorship/history unacquired"
SYNC_MARKER = "Wikidot sync (rev. 12)"
FILE_MARKER = "Wikidot sync (file)"


class FakeReplica:
    """In-memory Deepwell: pages by slug, files by page ID, uploaded blobs."""

    def __init__(self, pages, files=None):
        self.pages = {
            slug: {
                "page_id": i + 1,
                "revision_id": 100 + i,
                "slug": slug,
                "revision_user_id": -1,
                "revision_comments": SYNC_MARKER,
                "wikitext": "old",
                "title": slug,
                "tags": [],
            }
            for i, slug in enumerate(pages)
        }
        self.files = files or {}
        self.blobs, self.calls = {}, []
        self.page_get_queries = []

    def slug_of(self, page_id):
        [slug] = [
            slug for slug, page in self.pages.items() if page["page_id"] == page_id
        ]
        return slug

    def rpc(self, method, params):
        self.calls.append(method)
        if method == "page_get":
            self.page_get_queries.append(params)
            return self.pages.get(params["page"])
        if method == "page_move":
            page = self.pages.pop(self.slug_of(params["page"]))
            page["slug"] = params["new_slug"]
            page["revision_comments"] = params["revision_comments"]
            self.pages[params["new_slug"]] = page
            return {}
        if method == "page_import":
            slug = params["slug"]
            self.pages[slug] = {
                "page_id": len(self.pages) + 1,
                "revision_id": 201,
                "slug": slug,
                "revision_user_id": params["user_id"],
                "revision_comments": params["revision_comments"],
                "wikitext": params["wikitext"],
                "title": params["title"],
                "tags": params["tags"],
            }
            return {}
        if method == "page_edit":
            page = self.pages[self.slug_of(params["page"])]
            page.update(
                {
                    key: params[key]
                    for key in ("wikitext", "title", "tags")
                    if key in params
                }
            )
            page["revision_comments"] = params["revision_comments"]
            return {}
        if method == "page_get_files":
            return list(self.files.get(params["page_id"], {}).values())
        if method == "blob_upload":
            blob = f"blob{len(self.blobs)}"
            self.blobs[blob] = None
            return {"presign_url": blob, "pending_blob_id": blob}
        page_files = self.files.setdefault(params["page_id"], {})
        by_id = {file["file_id"]: name for name, file in page_files.items()}
        if method == "file_create":
            name = params["name"]
            page_files[name] = self.file(
                name, self.blobs[params["uploaded_blob_id"]], 500 + len(page_files)
            )
        elif method == "file_edit":
            name = by_id[params["file_id"]]
            page_files[name] = self.file(
                name, self.blobs[params["uploaded_blob_id"]], params["file_id"]
            )
        elif method == "file_delete":
            del page_files[by_id[params["file"]]]
        else:
            raise AssertionError(method)
        return {}

    def put(self, url, data):
        self.blobs[url] = data

    @staticmethod
    def file(name, data, file_id):
        return {
            "name": name,
            "file_id": file_id,
            "revision_id": file_id * 10,
            "size": len(data),
            "s3_hash": hashlib.sha512(data).hexdigest(),
            "revision_user_id": -1,
            "revision_comments": FILE_MARKER,
        }


def files_listing(files):
    """A one-page PageFilesModule body listing these {name: bytes}."""
    rows = "".join(
        f'<tr id="file-row-{i}"><td><a href="/local--files/icons/{name}">{name}</a></td>'
        f'<td><span title="data">data</span></td><td>{format_size(len(data))}</td></tr>'
        for i, (name, data) in enumerate(files.items())
    )
    return f"<p>Total files: {len(files)}</p><table>{rows}</table>"


class OwnershipTest(unittest.TestCase):
    def test_link_migration_stays_refreshable_only_for_exact_marker_and_actor(self):
        marker = "Cobalt page-link migration " + "a" * 64
        for actor, comment, editable in [
            (-1, marker, True),
            (42, marker, False),
            (-1, marker + " extra", False),
            (-1, "Cobalt page-link migration short", False),
        ]:
            with self.subTest(actor=actor, comment=comment):
                replica = FakeReplica(["story"])
                page = replica.pages["story"]
                page.update(revision_user_id=actor, revision_comments=comment)
                before = page["wikitext"]
                sync_page(
                    replica,
                    6000000,
                    -1,
                    "story",
                    {"title": "New", "source": "refreshed", "tags": []},
                    SYNC_MARKER,
                )
                self.assertEqual(page["wikitext"], "refreshed" if editable else before)

    def test_newer_local_page_is_kept_without_rpc_write(self):
        replica = FakeReplica(["story"])
        page = replica.pages["story"]
        page.update(revision_user_id=42, revision_comments=SYNC_MARKER)
        before = page.copy()
        state = {"title": "New", "source": "source", "tags": []}
        outcome = sync_page(
            replica, 6000000, -1, "story", state, "Wikidot sync (rev. 13)"
        )
        self.assertIn("kept:", outcome)
        self.assertEqual(replica.pages["story"], before)
        self.assertNotIn("page_edit", replica.calls)

    def test_missing_page_revision_metadata_is_kept(self):
        replica = FakeReplica(["story"])
        del replica.pages["story"]["revision_comments"]
        outcome = sync_page(
            replica,
            6000000,
            -1,
            "story",
            {"title": "New", "source": "source", "tags": []},
            SYNC_MARKER,
        )
        self.assertIn("kept:", outcome)
        self.assertNotIn("page_edit", replica.calls)

    def test_import_owned_page_and_rename_can_change(self):
        replica = FakeReplica(["old"])
        replica.pages["old"]["revision_comments"] = POC_MARKER
        rename = {"from": "old", "to": "story", "revision": 13}
        sync_renames(replica, 6000000, -1, [rename])
        self.assertEqual(rename["outcome"], "moved")
        outcome = sync_page(
            replica,
            6000000,
            -1,
            "story",
            {"title": "New", "source": "source", "tags": []},
            SYNC_MARKER,
        )
        self.assertIn("edited", outcome)
        self.assertEqual(replica.pages["story"]["wikitext"], "source")

    def test_unowned_rename_blocks_move(self):
        replica = FakeReplica(["old"])
        replica.pages["old"]["revision_comments"] = "Human edit"
        rename = {"from": "old", "to": "story", "revision": 13}
        sync_renames(replica, 6000000, -1, [rename])
        self.assertIn("kept:", rename["outcome"])
        self.assertNotIn("page_move", replica.calls)
        self.assertEqual(list(replica.pages), ["old"])

    def test_refused_page_has_no_date_update_or_duplicate_rename_target(self):
        replica = FakeReplica(["old", "local"])
        replica.pages["old"].update(revision_user_id=42, revision_comments="Human edit")
        changes = [
            {
                "slug": "new",
                "revision": revision,
                "flags": "R",
                "changed_at": revision,
                "comments": f'You successfully renamed the page: "{old}" to "{new}".',
                "title": "New",
                "user_slug": "u",
                "user_name": "U",
                "user_id": 1,
            }
            for old, new, revision in [("old", "middle", 12), ("middle", "new", 13)]
        ]
        state = {
            "status": "ok",
            "page_id": 1,
            "title": "New",
            "source": "source",
            "tags": [],
            "created_at": 1,
            "updated_at": 2,
        }
        events = {"new": {"touched": {"a.jpg"}, "gone": set(), "unrecognized": []}}
        with (
            tempfile.TemporaryDirectory() as tmp,
            mock.patch.object(wikidot_sync, "LoopbackRpc") as factory,
            mock.patch.object(wikidot_sync, "paced", return_value=changes),
            mock.patch.object(wikidot_sync, "wikidot_page", return_value=state),
            mock.patch.object(wikidot_sync, "file_events", return_value=events),
            mock.patch.object(
                wikidot_sync,
                "module",
                side_effect=AssertionError("protected files reached"),
            ),
        ):
            password_file = Path(tmp) / "password"
            password_file.write_text("password")
            factory.side_effect = [
                mock.Mock(rpc=mock.Mock(return_value={"session_token": "token"})),
                replica,
            ]
            original_rpc = replica.rpc
            replica.rpc = lambda method, params: (
                {"user_id": -1}
                if method == "session_get"
                else original_rpc(method, params)
            )
            wikidot_sync.main(
                [
                    "https://source",
                    "https://target",
                    "http://127.0.0.1:1/jsonrpc",
                    "6000000",
                    str(password_file),
                    "0",
                    tmp,
                ]
            )
            report = json.loads((Path(tmp) / "sync-report.json").read_text())
            sql = (Path(tmp) / "sync-apply.sql").read_text()
        self.assertIn("kept:", report["renames"][0]["outcome"])
        self.assertIn("kept:", report["pages"]["new"])
        self.assertEqual(report["files"]["new"]["skipped"], "page not synced")
        self.assertNotIn("page_get_files", replica.calls)
        self.assertNotIn("new", replica.pages)
        self.assertNotIn("UPDATE page SET", sql)
        self.assertNotIn("page_import", replica.calls)


class LinkSyncTest(unittest.TestCase):
    def run_page_sync(self, replica, state):
        changes = [
            {
                "slug": "story",
                "revision": 13,
                "flags": "S",
                "changed_at": 13,
                "comments": "",
                "title": "Story",
                "user_slug": "u",
                "user_name": "U",
                "user_id": 1,
            }
        ]
        with (
            tempfile.TemporaryDirectory() as tmp,
            mock.patch.object(wikidot_sync, "LoopbackRpc") as factory,
            mock.patch.object(wikidot_sync, "paced", return_value=changes),
            mock.patch.object(wikidot_sync, "wikidot_page", return_value=state),
            mock.patch.object(wikidot_sync, "file_events", return_value={}),
        ):
            password_file = Path(tmp) / "password"
            password_file.write_text("password")
            factory.side_effect = [
                mock.Mock(rpc=mock.Mock(return_value={"session_token": "token"})),
                replica,
            ]
            original_rpc = replica.rpc
            replica.rpc = lambda method, params: (
                {"user_id": -1}
                if method == "session_get"
                else original_rpc(method, params)
            )
            wikidot_sync.main(
                [
                    "https://source",
                    "https://target",
                    "http://127.0.0.1:1/jsonrpc",
                    "6000000",
                    str(password_file),
                    "0",
                    tmp,
                ]
            )
            return json.loads((Path(tmp) / "sync-report.json").read_text())

    def test_changed_page_translates_only_confirmed_active_exact_slug(self):
        source = (
            "[[[https://source/character%3Aally?mode=1#bio|Ally]]] "
            "https://source/character%3Aally#next "
            "[[[https://source/absent|Absent]]] "
            "[[[https://source/renamed|Renamed]]] "
            "[[[https://source/deleted|Deleted]]] "
            "[!-- https://source/character%3Aally --]"
        )
        state = {
            "status": "ok",
            "page_id": 11,
            "title": "Story",
            "source": source,
            "tags": [],
            "created_at": 1,
            "updated_at": 2,
        }
        replica = FakeReplica(["story", "character:ally", "renamed", "deleted"])
        replica.pages["renamed"]["slug"] = "different-name"
        replica.pages["deleted"]["deleted_at"] = 123
        report = self.run_page_sync(replica, state)
        self.assertEqual(
            replica.pages["story"]["wikitext"],
            source.replace(
                "https://source/character%3Aally", "https://target/character%3Aally", 2
            ),
        )
        self.assertEqual(state["source"], source)
        self.assertEqual(
            report["link_translation"],
            {
                "rewritten": 2,
                "unconfirmed": 3,
                "unsafe_context": 1,
            },
        )
        candidates = [
            query for query in replica.page_get_queries if query["page"] != "story"
        ]
        self.assertEqual(
            {query["page"] for query in candidates},
            {"character:ally", "absent", "renamed", "deleted"},
        )
        self.assertEqual(len(candidates), 4)
        self.assertTrue(all(query["site_id"] == 6000000 for query in candidates))

    def test_human_owned_page_remains_unmodified(self):
        source = "[[[https://source/character%3Aally|Ally]]]"
        state = {
            "status": "ok",
            "page_id": 11,
            "title": "Story",
            "source": source,
            "tags": [],
            "created_at": 1,
            "updated_at": 2,
        }
        replica = FakeReplica(["story", "character:ally"])
        replica.pages["story"].update(
            revision_user_id=42, revision_comments="Human edit"
        )
        before = replica.pages["story"].copy()
        report = self.run_page_sync(replica, state)
        self.assertEqual(replica.pages["story"], before)
        self.assertEqual(state["source"], source)
        self.assertIn("kept:", report["pages"]["story"])
        self.assertNotIn("page_edit", replica.calls)


class SyncFilesTest(unittest.TestCase):
    def run_sync(self, replica, wikidot_files, events):
        downloads = []

        def request(url):
            downloads.append(url)
            return 200, wikidot_files[url.rsplit("/", 1)[1]]

        listing = {"status": "ok", "body": files_listing(wikidot_files)}
        with (
            mock.patch.object(wikidot_sync, "module", return_value=listing),
            mock.patch.object(wikidot_sync, "wikidot_request", side_effect=request),
        ):
            outcome = sync_files(
                replica,
                6000000,
                -1,
                "https://cobalt-company.wikidot.com",
                1,
                1312278321,
                events,
            )
        return outcome, downloads

    def test_adds_replaces_deletes_and_is_idempotent(self):
        new, changed, kept = b"e" * 30274, b"v2" * 700, b"k" * 2048
        replica = FakeReplica(
            ["icons"],
            files={
                1: {
                    "icon_valentine.jpg": FakeReplica.file(
                        "icon_valentine.jpg", b"v1" * 700, 7
                    ),
                    "icon_kept.jpg": FakeReplica.file("icon_kept.jpg", kept, 8),
                    "icon_brynnal.jpg": FakeReplica.file("icon_brynnal.jpg", b"b", 9),
                    "icon_extra.jpg": FakeReplica.file("icon_extra.jpg", b"x", 10),
                }
            },
        )
        wikidot = {
            "icon_emdee.jpg": new,
            "icon_valentine.jpg": changed,
            "icon_kept.jpg": kept,
        }
        events = {
            "touched": {"icon_emdee.jpg", "icon_valentine.jpg"},
            "gone": {"icon_brynnal.jpg"},
            "unrecognized": [],
        }

        outcome, downloads = self.run_sync(replica, wikidot, events)
        self.assertEqual(
            outcome,
            {
                "icon_brynnal.jpg": "deleted",
                "icon_emdee.jpg": "created",
                "icon_extra.jpg": "kept: absent on Wikidot, but no revision in the window removed it",
                "icon_valentine.jpg": "updated",
            },
        )
        self.assertEqual(
            downloads,
            [
                "https://cobalt-company.wdfiles.com/local--files/icons/icon_emdee.jpg",
                "https://cobalt-company.wdfiles.com/local--files/icons/icon_valentine.jpg",
            ],
        )
        self.assertEqual(
            {name: file["s3_hash"] for name, file in replica.files[1].items()},
            {
                name: hashlib.sha512(data).hexdigest()
                for name, data in {**wikidot, "icon_extra.jpg": b"x"}.items()
            },
        )

        again, _ = self.run_sync(replica, wikidot, events)
        self.assertEqual(
            again,
            {
                "icon_emdee.jpg": "unchanged",
                "icon_extra.jpg": "kept: absent on Wikidot, but no revision in the window removed it",
                "icon_valentine.jpg": "unchanged",
            },
        )

    def test_existing_local_file_kept_even_when_filename_matches(self):
        replica = FakeReplica(
            ["icons"], files={1: {"a.jpg": FakeReplica.file("a.jpg", b"local", 7)}}
        )
        replica.files[1]["a.jpg"].update(
            revision_user_id=42, revision_comments="Human edit"
        )
        before = replica.files[1]["a.jpg"].copy()
        outcome, downloads = self.run_sync(
            replica,
            {"a.jpg": b"source"},
            {"touched": {"a.jpg"}, "gone": set(), "unrecognized": []},
        )
        self.assertIn("kept:", outcome["a.jpg"])
        self.assertEqual(replica.files[1]["a.jpg"], before)
        self.assertEqual(downloads, [])
        self.assertNotIn("blob_upload", replica.calls)

    def test_unowned_file_is_not_deleted(self):
        replica = FakeReplica(
            ["icons"], files={1: {"b.jpg": FakeReplica.file("b.jpg", b"local", 8)}}
        )
        replica.files[1]["b.jpg"]["revision_comments"] = "Human edit"
        outcome, _ = self.run_sync(
            replica, {}, {"touched": set(), "gone": {"b.jpg"}, "unrecognized": []}
        )
        self.assertIn("kept:", outcome["b.jpg"])
        self.assertIn("b.jpg", replica.files[1])
        self.assertNotIn("file_delete", replica.calls)

    def test_import_owned_file_can_be_updated_and_deleted(self):
        replica = FakeReplica(
            ["icons"],
            files={
                1: {
                    "a.jpg": FakeReplica.file("a.jpg", b"old", 7),
                    "b.jpg": FakeReplica.file("b.jpg", b"old", 8),
                }
            },
        )
        replica.files[1]["a.jpg"]["revision_comments"] = POC_MARKER
        outcome, _ = self.run_sync(
            replica,
            {"a.jpg": b"new"},
            {"touched": {"a.jpg"}, "gone": {"b.jpg"}, "unrecognized": []},
        )
        self.assertEqual(outcome, {"a.jpg": "updated", "b.jpg": "deleted"})

    def test_missing_file_metadata_keeps_and_reports_conflict(self):
        replica = FakeReplica(
            ["icons"], files={1: {"a.jpg": FakeReplica.file("a.jpg", b"old", 7)}}
        )
        del replica.files[1]["a.jpg"]["revision_user_id"]
        outcome, downloads = self.run_sync(
            replica,
            {"a.jpg": b"new"},
            {"touched": {"a.jpg"}, "gone": set(), "unrecognized": []},
        )
        self.assertIn("kept:", outcome["a.jpg"])
        self.assertEqual(downloads, [])

    def test_missing_file_page_is_reported_not_created(self):
        replica = FakeReplica(["icons"])
        events = {"touched": set(), "gone": set(), "unrecognized": []}
        listing = {
            "status": "ok",
            "body": '<p>Total files: 1</p><tr id="file-row-1"><td><a href="/local--files/icons/a.jpg">'
            "a.jpg</a></td><td><span>x</span></td><td>29.56 kB</td></tr>",
        }
        missing = b"<html><head><title>The file does not exist</title></head></html>"
        with (
            mock.patch.object(wikidot_sync, "module", return_value=listing),
            mock.patch.object(
                wikidot_sync, "wikidot_request", return_value=(200, missing)
            ),
        ):
            outcome = sync_files(
                replica, 6000000, -1, "https://cobalt-company.wikidot.com", 1, 5, events
            )
        self.assertEqual(
            outcome, {"a.jpg": "skipped: Wikidot says the file does not exist"}
        )
        self.assertNotIn("file_create", replica.calls)

    def test_incomplete_listing_changes_nothing(self):
        replica = FakeReplica(
            ["icons"], files={1: {"a.jpg": FakeReplica.file("a.jpg", b"a", 7)}}
        )
        listing = {"status": "ok", "body": "<p>Total files: 468</p>"}
        with (
            mock.patch.object(wikidot_sync, "module", return_value=listing),
            self.assertRaisesRegex(wikidot_sync.FileListError, "listed 0 of 468"),
        ):
            sync_files(
                replica,
                6000000,
                -1,
                "https://cobalt-company.wikidot.com",
                1,
                5,
                {"touched": set(), "gone": {"a.jpg"}, "unrecognized": []},
            )
        self.assertEqual(list(replica.files[1]), ["a.jpg"])


class FileUrlTest(unittest.TestCase):
    def test_encodes_like_wikidot_redirect(self):
        # wdfiles.com answers 500 for this file with a raw colon, 200 with %3A.
        self.assertEqual(
            wikidot_sync.file_url(
                "https://cobalt-company.wikidot.com",
                "/local--files/writing:2026-09-14-carrot-cake-and-a-luckydo/Baird%20Cosmology.png",
            ),
            "https://cobalt-company.wdfiles.com/local--files/"
            "writing%3A2026-09-14-carrot-cake-and-a-luckydo/Baird%20Cosmology.png",
        )


class SyncRenamesTest(unittest.TestCase):
    def test_moves_in_order_and_reruns_as_already_moved(self):
        replica = FakeReplica(["character:brynnal", "writing:x", "writing:y"])
        renames = [
            {"from": "character:brynnal", "to": "character:bryn", "revision": 117},
            {"from": "character:bryn", "to": "character:baird", "revision": 118},
            {"from": "writing:x", "to": "writing:y", "revision": 3},
        ]
        sync_renames(replica, 6000000, -1, renames)
        self.assertEqual(
            [r["outcome"] for r in renames],
            ["moved", "moved", "skip: both slugs exist on the replica"],
        )
        self.assertEqual(
            sorted(replica.pages), ["character:baird", "writing:x", "writing:y"]
        )

        sync_renames(replica, 6000000, -1, renames)
        # A rerun finds the page at the end of the chain; nothing moves again.
        self.assertEqual(
            [r["outcome"] for r in renames][:2],
            [wikidot_sync.NOTHING_TO_MOVE, "already moved"],
        )
        self.assertEqual(
            sorted(replica.pages), ["character:baird", "writing:x", "writing:y"]
        )


if __name__ == "__main__":
    unittest.main()
