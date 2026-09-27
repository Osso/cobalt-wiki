import hashlib
import unittest

from tools.cobalt_migration.current_link_migration import apply_current_links

SOURCE = "https://cobalt-company.wikidot.com"
TARGET = "https://cobalt-company.sakuin.org"
OWNER = "Wikidot sync (rev. 12)"


def digest(source):
    return hashlib.sha256(source.encode("utf-8")).hexdigest()


class Store:
    def __init__(self):
        self.pages = {}
        self.history = {}
        self.drafts = {}
        self.notifications = []
        self.edits = []
        self.before_edit = None

    def add(self, page_id, slug, text, owner=OWNER):
        page = {
            "page_id": page_id,
            "slug": slug,
            "revision_id": page_id * 10,
            "revision_user_id": -1,
            "revision_comments": owner,
            "wikitext": text,
        }
        self.pages[page_id] = page
        self.history[page_id] = [text]
        self.drafts[page_id] = "unfinished draft"
        return page

    def rpc(self, method, params):
        if method == "page_get":
            assert params["details"] == {"wikitext": True}
            identifier = params["page"]
            if isinstance(identifier, int):
                return self.pages.get(identifier)
            return next(
                (p for p in self.pages.values() if p["slug"] == identifier), None
            )
        if method == "page_edit":
            if self.before_edit:
                action, self.before_edit = self.before_edit, None
                action(self)
            page = self.pages[params["page"]]
            if page["revision_id"] != params["last_revision_id"]:
                raise RuntimeError("revision conflict")
            if not params["preserve_draft"]:
                self.drafts.pop(page["page_id"], None)
            if not params["do_not_notify_watchers"]:
                self.notifications.append(page["page_id"])
            page["revision_id"] += 1
            page["revision_user_id"] = params["user_id"]
            page["revision_comments"] = params["revision_comments"]
            page["wikitext"] = params["wikitext"]
            self.history[page["page_id"]].append(page["wikitext"])
            self.edits.append(dict(params))
            return {"revision_id": page["revision_id"]}
        raise AssertionError(method)


def entry(page, after, count=1):
    return {
        "page_id": page["page_id"],
        "revision_id": page["revision_id"],
        "import_owned": True,
        "counts": {"rewritten": count},
        "before_sha256": digest(page["wikitext"]),
        "after_sha256": digest(after),
    }


def apply(store, plan):
    return apply_current_links(store, 6000000, -1, SOURCE, TARGET, plan)


class CurrentLinkMigrationTest(unittest.TestCase):
    def setUp(self):
        self.store = Store()
        self.destination = self.store.add(2, "scene", "Destination")
        self.before = f"See [{SOURCE}/scene Read]"
        self.after = f"See [{TARGET}/scene Read]"
        self.page = self.store.add(1, "story", self.before)
        self.plan = [entry(self.page, self.after)]

    def test_exact_edit_readback_preserves_history_and_draft_without_notification(self):
        receipt = apply(self.store, self.plan)
        self.assertEqual(self.store.history[1], [self.before, self.after])
        self.assertEqual(self.store.history[2], ["Destination"])
        self.assertEqual(self.store.drafts[1], "unfinished draft")
        self.assertEqual(self.store.notifications, [])
        self.assertEqual(
            self.store.edits[0]["revision_comments"],
            "Cobalt page-link migration " + digest(self.before),
        )
        self.assertEqual(
            receipt,
            [
                {
                    "page_id": 1,
                    "before_revision_id": 10,
                    "after_revision_id": 11,
                    "before_sha256": digest(self.before),
                    "after_sha256": digest(self.after),
                    "rewritten": 1,
                }
            ],
        )

    def test_preflight_all_pages_before_first_edit(self):
        second = self.store.add(3, "second", self.before, owner="local change")
        with self.assertRaises(ValueError):
            apply(self.store, self.plan + [entry(second, self.after)])
        self.assertEqual(self.store.edits, [])
        self.assertEqual(self.store.history[1], [self.before])

    def test_refuses_new_local_edit_even_with_matching_source(self):
        self.page.update(revision_id=11, revision_user_id=42, revision_comments="local")
        with self.assertRaises(ValueError):
            apply(self.store, self.plan)
        self.assertEqual(self.store.edits, [])

    def test_refuses_source_hash_drift(self):
        self.page["wikitext"] += " changed"
        with self.assertRaises(ValueError):
            apply(self.store, self.plan)
        self.assertEqual(self.store.edits, [])

    def test_refuses_missing_reviewed_page(self):
        del self.store.pages[1]
        with self.assertRaises(ValueError):
            apply(self.store, self.plan)
        self.assertEqual(self.store.edits, [])

    def test_refuses_unchanged_page(self):
        page = self.store.add(3, "plain", "No links")
        unchanged = entry(page, "No links", count=0)
        with self.assertRaises(ValueError):
            apply(self.store, [unchanged])
        self.assertEqual(self.store.edits, [])

    def test_refuses_missing_destination(self):
        del self.store.pages[2]
        with self.assertRaises(ValueError):
            apply(self.store, self.plan)
        self.assertEqual(self.store.edits, [])

    def test_refuses_destination_slug_mismatch(self):
        self.destination["slug"] = "renamed"
        with self.assertRaises(ValueError):
            apply(self.store, self.plan)
        self.assertEqual(self.store.edits, [])

    def test_refuses_alias_that_resolves_to_different_active_slug(self):
        self.destination["slug"] = "renamed"
        original_rpc = self.store.rpc

        def rpc(method, params):
            if method == "page_get" and params["page"] == "scene":
                return self.destination
            return original_rpc(method, params)

        self.store.rpc = rpc
        with self.assertRaises(ValueError):
            apply(self.store, self.plan)
        self.assertEqual(self.store.edits, [])

    def test_refuses_after_hash_and_rewrite_count_mismatch(self):
        for key, value in (
            ("after_sha256", digest("wrong")),
            ("counts", {"rewritten": 2}),
        ):
            with self.subTest(key=key):
                wrong = dict(self.plan[0], **{key: value})
                with self.assertRaises(ValueError):
                    apply(self.store, [wrong])
                self.assertEqual(self.store.edits, [])

    def test_refuses_unowned_reviewed_entry(self):
        self.plan[0]["import_owned"] = False
        with self.assertRaises(ValueError):
            apply(self.store, self.plan)
        self.assertEqual(self.store.edits, [])

    def test_rechecks_target_and_source_before_each_write(self):
        second = self.store.add(3, "second", self.before)
        original_rpc = self.store.rpc
        calls = 0

        def rpc(method, params):
            nonlocal calls
            if method == "page_get" and params["page"] == 3:
                calls += 1
                if calls == 2:
                    self.store.pages[3]["wikitext"] += " concurrent"
            return original_rpc(method, params)

        self.store.rpc = rpc
        with self.assertRaises(ValueError):
            apply(self.store, self.plan + [entry(second, self.after)])
        self.assertEqual(len(self.store.edits), 1)
        self.assertEqual(self.store.history[3], [self.before])

    def test_rechecks_destination_before_write(self):
        original_rpc = self.store.rpc
        reads = 0

        def rpc(method, params):
            nonlocal reads
            if method == "page_get" and params["page"] == "scene":
                reads += 1
                if reads == 2:
                    del self.store.pages[2]
            return original_rpc(method, params)

        self.store.rpc = rpc
        with self.assertRaises(ValueError):
            apply(self.store, self.plan)
        self.assertEqual(self.store.edits, [])

    def test_stops_on_rpc_revision_conflict(self):
        self.store.before_edit = lambda store: store.pages[1].update(revision_id=11)
        with self.assertRaises(RuntimeError):
            apply(self.store, self.plan)
        self.assertEqual(self.store.history[1], [self.before])

    def test_stops_when_readback_differs(self):
        original_rpc = self.store.rpc

        def rpc(method, params):
            result = original_rpc(method, params)
            if method == "page_edit":
                self.store.pages[1]["wikitext"] = "unexpected saved source"
            return result

        self.store.rpc = rpc
        with self.assertRaises(ValueError):
            apply(self.store, self.plan)
        self.assertEqual(len(self.store.edits), 1)


if __name__ == "__main__":
    unittest.main()
