import hashlib
import unittest
from unittest import mock

from tools.cobalt_migration import wikidot_sync
from tools.cobalt_migration.wikidot_files import format_size
from tools.cobalt_migration.wikidot_sync import (
    apply_sql,
    decode_view_source,
    deletion_candidates,
    missing_on_wikidot,
    parse_meta,
    parse_renames,
    plan_rename,
    sync_deletions,
    sync_files,
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
            [{"slug": "writing:it's", "title": "It's", "revision": 0, "flags": "N",
              "changed_at": 1, "user_slug": "allicat", "user_name": "Allicat",
              "user_id": 7570574, "comments": ""}],
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

    def test_apply_sql_moves_rows_to_renamed_pages_and_drops_deleted_pages(self):
        sql = apply_sql(
            6000000, {}, [],
            renames=[
                {"from": "character:brynnal", "to": "character:baird", "outcome": "moved"},
                {"from": "a", "to": "b", "outcome": "already moved"},
                {"from": "c", "to": "d", "outcome": "skip: both slugs exist on the replica"},
                {"from": "e", "to": "f", "outcome": wikidot_sync.NOTHING_TO_MOVE},
            ],
            deleted=["writing:gone"],
        )
        self.assertIn(
            "UPDATE wikidot_site_change SET page_slug = 'character:baird' "
            "WHERE site_id = 6000000 AND page_slug = 'character:brynnal';",
            sql,
        )
        self.assertIn("SET page_slug = 'b' WHERE site_id = 6000000 AND page_slug = 'a';", sql)
        self.assertNotIn("page_slug = 'c'", sql)
        self.assertIn("SET page_slug = 'f' WHERE site_id = 6000000 AND page_slug = 'e';", sql)
        self.assertIn(
            "DELETE FROM wikidot_site_change WHERE site_id = 6000000 AND page_slug = 'writing:gone';",
            sql,
        )


def change(slug, flags, changed_at, revision, comments):
    return {"slug": slug, "flags": flags, "changed_at": changed_at, "revision": revision,
            "comments": comments}


# Rows as SiteChanges lists them (newest first, under the page's current slug).
RENAME_ROWS = [
    change("writing:2026-09-18-a-collection-of-thought-exercises", "R", 1789804618, 4,
           'You successfully renamed the page: "writing:2028-09-18-a-collection-of-thought-exercises" '
           'to "writing:2026-09-18-a-collection-of-thought-exercises".'),
    change("character:baird", "S", 1789090700, 119, ""),
    change("character:baird", "R", 1789090609, 118,
           'You successfully renamed the page: "character:brynnal" to "character:baird".'),
]

# Wikidot's answer for a page that does not exist (HTTP 404), trimmed.
PAGE_DOES_NOT_EXIST = """<div id="page-content">
<p>The page <em>writing:gone</em> you want to access does not exist.</p>
<ul><li><a href="javascript:;" onclick="WIKIDOT.page.listeners.createPageClick(event)">create page</a></li></ul>
</div>"""


class RenameTest(unittest.TestCase):
    def test_reads_old_and_new_names_oldest_first(self):
        self.assertEqual(
            [(r["from"], r["to"], r["revision"]) for r in parse_renames(RENAME_ROWS)],
            [
                ("character:brynnal", "character:baird", 118),
                ("writing:2028-09-18-a-collection-of-thought-exercises",
                 "writing:2026-09-18-a-collection-of-thought-exercises", 4),
            ],
        )

    def test_unrecognized_rename_comment_has_no_names(self):
        [rename] = parse_renames([change("x", "R", 1, 2, "Renamed by magic")])
        self.assertEqual((rename["from"], rename["to"]), (None, None))
        self.assertEqual(plan_rename(rename, True, False), "skip: rename comment not recognized")

    def test_decision_by_replica_state(self):
        rename = {"from": "character:brynnal", "to": "character:baird"}
        self.assertEqual(plan_rename(rename, True, False), "move")
        self.assertEqual(plan_rename(rename, False, True), "already moved")
        self.assertEqual(plan_rename(rename, True, True), "skip: both slugs exist on the replica")
        self.assertTrue(plan_rename(rename, False, False).startswith("nothing to move"))


class DeletionEvidenceTest(unittest.TestCase):
    def test_candidates_are_replica_pages_missing_from_wikidot(self):
        self.assertEqual(
            deletion_candidates(
                ["start", "writing:gone", "character:baird", "character:brynnal"],
                ["start", "character:baird"],
                handled={"character:brynnal"},
            ),
            ["writing:gone"],
        )

    def test_only_a_404_saying_the_page_does_not_exist_counts(self):
        self.assertTrue(missing_on_wikidot(404, PAGE_DOES_NOT_EXIST, "writing:gone"))
        self.assertFalse(missing_on_wikidot(200, PAGE_DOES_NOT_EXIST, "writing:gone"))
        self.assertFalse(missing_on_wikidot(404, PAGE_DOES_NOT_EXIST, "writing:other"))
        self.assertFalse(missing_on_wikidot(404, "<h1>Service unavailable</h1>", "writing:gone"))


class FakeReplica:
    """In-memory Deepwell: pages by slug, files by page ID, uploaded blobs."""

    def __init__(self, pages, files=None, creators=None):
        self.pages = {slug: {"page_id": i + 1, "revision_id": 100 + i, "slug": slug}
                      for i, slug in enumerate(pages)}
        self.creators = creators or {}
        self.files = files or {}
        self.blobs, self.calls = {}, []

    def slug_of(self, page_id):
        [slug] = [slug for slug, page in self.pages.items() if page["page_id"] == page_id]
        return slug

    def rpc(self, method, params):
        self.calls.append(method)
        if method == "page_get":
            return self.pages.get(params["page"])
        if method == "page_move":
            self.pages[params["new_slug"]] = self.pages.pop(self.slug_of(params["page"]))
            return {}
        if method == "page_delete":
            del self.pages[self.slug_of(params["page"])]
            return {}
        if method == "page_revision_get":
            return {"user_id": self.creators.get(self.slug_of(params["page_id"]), -1)}
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
            page_files[name] = self.file(name, self.blobs[params["uploaded_blob_id"]], 500 + len(page_files))
        elif method == "file_edit":
            name = by_id[params["file_id"]]
            page_files[name] = self.file(name, self.blobs[params["uploaded_blob_id"]], params["file_id"])
        elif method == "file_delete":
            del page_files[by_id[params["file"]]]
        else:
            raise AssertionError(method)
        return {}

    def put(self, url, data):
        self.blobs[url] = data

    @staticmethod
    def file(name, data, file_id):
        return {"name": name, "file_id": file_id, "revision_id": file_id * 10, "size": len(data),
                "s3_hash": hashlib.sha512(data).hexdigest()}


def files_listing(files):
    """A one-page PageFilesModule body listing these {name: bytes}."""
    rows = "".join(
        f'<tr id="file-row-{i}"><td><a href="/local--files/icons/{name}">{name}</a></td>'
        f'<td><span title="data">data</span></td><td>{format_size(len(data))}</td></tr>'
        for i, (name, data) in enumerate(files.items())
    )
    return f"<p>Total files: {len(files)}</p><table>{rows}</table>"


class SyncFilesTest(unittest.TestCase):
    def run_sync(self, replica, wikidot_files, events):
        downloads = []

        def request(url):
            downloads.append(url)
            return 200, wikidot_files[url.rsplit("/", 1)[1]]

        listing = {"status": "ok", "body": files_listing(wikidot_files)}
        with mock.patch.object(wikidot_sync, "module", return_value=listing), \
                mock.patch.object(wikidot_sync, "wikidot_request", side_effect=request):
            outcome = sync_files(replica, 6000000, -1, "https://cobalt-company.wikidot.com",
                                 1, 1312278321, events)
        return outcome, downloads

    def test_adds_replaces_deletes_and_is_idempotent(self):
        new, changed, kept = b"e" * 30274, b"v2" * 700, b"k" * 2048
        replica = FakeReplica(["icons"], files={1: {
            "icon_valentine.jpg": FakeReplica.file("icon_valentine.jpg", b"v1" * 700, 7),
            "icon_kept.jpg": FakeReplica.file("icon_kept.jpg", kept, 8),
            "icon_brynnal.jpg": FakeReplica.file("icon_brynnal.jpg", b"b", 9),
            "icon_extra.jpg": FakeReplica.file("icon_extra.jpg", b"x", 10),
        }})
        wikidot = {"icon_emdee.jpg": new, "icon_valentine.jpg": changed, "icon_kept.jpg": kept}
        events = {"touched": {"icon_emdee.jpg", "icon_valentine.jpg"}, "gone": {"icon_brynnal.jpg"},
                  "unrecognized": []}

        outcome, downloads = self.run_sync(replica, wikidot, events)
        self.assertEqual(outcome, {
            "icon_brynnal.jpg": "deleted",
            "icon_emdee.jpg": "created",
            "icon_extra.jpg": "kept: absent on Wikidot, but no revision in the window removed it",
            "icon_valentine.jpg": "updated",
        })
        self.assertEqual(downloads, [
            "https://cobalt-company.wdfiles.com/local--files/icons/icon_emdee.jpg",
            "https://cobalt-company.wdfiles.com/local--files/icons/icon_valentine.jpg",
        ])
        self.assertEqual(
            {name: file["s3_hash"] for name, file in replica.files[1].items()},
            {name: hashlib.sha512(data).hexdigest() for name, data in
             {**wikidot, "icon_extra.jpg": b"x"}.items()},
        )

        again, _ = self.run_sync(replica, wikidot, events)
        self.assertEqual(again, {
            "icon_emdee.jpg": "unchanged",
            "icon_extra.jpg": "kept: absent on Wikidot, but no revision in the window removed it",
            "icon_valentine.jpg": "unchanged",
        })

    def test_missing_file_page_is_reported_not_created(self):
        replica = FakeReplica(["icons"])
        events = {"touched": set(), "gone": set(), "unrecognized": []}
        listing = {
            "status": "ok",
            "body": '<p>Total files: 1</p><tr id="file-row-1"><td><a href="/local--files/icons/a.jpg">'
                    'a.jpg</a></td><td><span>x</span></td><td>29.56 kB</td></tr>',
        }
        missing = b"<html><head><title>The file does not exist</title></head></html>"
        with mock.patch.object(wikidot_sync, "module", return_value=listing), \
                mock.patch.object(wikidot_sync, "wikidot_request", return_value=(200, missing)):
            outcome = sync_files(replica, 6000000, -1, "https://cobalt-company.wikidot.com", 1, 5, events)
        self.assertEqual(outcome, {"a.jpg": "skipped: Wikidot says the file does not exist"})
        self.assertNotIn("file_create", replica.calls)

    def test_incomplete_listing_changes_nothing(self):
        replica = FakeReplica(["icons"], files={1: {"a.jpg": FakeReplica.file("a.jpg", b"a", 7)}})
        listing = {"status": "ok", "body": "<p>Total files: 468</p>"}
        with mock.patch.object(wikidot_sync, "module", return_value=listing):
            with self.assertRaisesRegex(wikidot_sync.FileListError, "listed 0 of 468"):
                sync_files(replica, 6000000, -1, "https://cobalt-company.wikidot.com", 1, 5,
                           {"touched": set(), "gone": {"a.jpg"}, "unrecognized": []})
        self.assertEqual(list(replica.files[1]), ["a.jpg"])


class SyncRenamesTest(unittest.TestCase):
    def test_moves_in_order_and_reruns_as_already_moved(self):
        replica = FakeReplica(["character:brynnal", "writing:x", "writing:y"])
        renames = [
            {"from": "character:brynnal", "to": "character:bryn", "revision": 117},
            {"from": "character:bryn", "to": "character:baird", "revision": 118},
            {"from": "writing:x", "to": "writing:y", "revision": 3},
        ]
        sync_renames(replica, 6000000, -1, renames)
        self.assertEqual([r["outcome"] for r in renames],
                         ["moved", "moved", "skip: both slugs exist on the replica"])
        self.assertEqual(sorted(replica.pages), ["character:baird", "writing:x", "writing:y"])

        sync_renames(replica, 6000000, -1, renames)
        # A rerun finds the page at the end of the chain; nothing moves again.
        self.assertEqual([r["outcome"] for r in renames][:2],
                         [wikidot_sync.NOTHING_TO_MOVE, "already moved"])
        self.assertEqual(sorted(replica.pages), ["character:baird", "writing:x", "writing:y"])


class SyncDeletionsTest(unittest.TestCase):
    def test_deletes_only_import_pages_wikidot_says_are_gone(self):
        replica = FakeReplica(["writing:gone", "writing:native", "writing:private"],
                              creators={"writing:native": 42})
        answers = {
            "https://cobalt-company.wikidot.com/writing:gone": (404, PAGE_DOES_NOT_EXIST.encode()),
            "https://cobalt-company.wikidot.com/writing:private": (200, b"<p>page</p>"),
        }
        with mock.patch.object(wikidot_sync, "wikidot_request", side_effect=answers.__getitem__):
            outcome = sync_deletions(replica, 6000000, -1, "https://cobalt-company.wikidot.com",
                                     ["writing:gone", "writing:native", "writing:private", "writing:absent"])
        self.assertEqual(outcome, {
            "writing:gone": "deleted",
            "writing:native": "kept: not created by the import principal",
            "writing:private": "kept: Wikidot answers http 200 without saying the page does not exist",
            "writing:absent": "already gone from the replica",
        })
        self.assertEqual(sorted(replica.pages), ["writing:native", "writing:private"])


if __name__ == "__main__":
    unittest.main()
