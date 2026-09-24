import copy
import unittest

from tests.cobalt_migration import test_poc_import as fixtures
from tools.cobalt_migration import poc_import as poc
from tools.cobalt_migration import poc_missing_inventory as missing


class TaggedStore(fixtures.Store):
    def rpc(self, method, params):
        result = super().rpc(method, params)
        if method == "page_import":
            result["tags"] = params["tags"]
        return result


def inventory(plan, store):
    pages = [
        {
            "page_id": p["page_id"],
            "slug": p["slug"],
            "deleted": False,
            "latest_revision_id": p["revision_id"],
        }
        for p in store.pages.values()
    ]
    files = [
        {"file_id": i + 1, "page_id": p, "name": n, "deleted": False}
        for i, (p, n) in enumerate(store.files)
    ]
    return {
        "schema": 1,
        "site_id": plan["site_id"],
        "plan_sha256": plan["plan_sha256"],
        "rpc_endpoint": "http://127.0.0.1:2749/jsonrpc",
        "pages": pages,
        "page_revisions": [],
        "files": files,
        "file_revisions": [],
        "orphan_audit_page_ids": [],
    }


class MissingApplyTests(unittest.TestCase):
    setUp = fixtures.ImportTests.setUp
    tearDown = fixtures.ImportTests.tearDown
    make_archive = fixtures.ImportTests.make_archive
    plan = fixtures.ImportTests.plan

    def apply(self, plan, store, snapshot=None, **limits):
        return missing.apply_missing(
            self.archive,
            self.path,
            snapshot or inventory(plan, store),
            store.rpc,
            store.put,
            endpoint="http://127.0.0.1:2749/jsonrpc",
            max_pages=limits.get("pages", 10),
            max_files=limits.get("files", 10),
        )

    def test_create_atomic_tags_and_resume_preserves_edits(self):
        plan, store = self.plan(), TaggedStore()
        self.assertTrue(
            callable(getattr(missing, "apply_missing", None)),
            "missing-only application is not implemented",
        )
        self.assertEqual(self.apply(plan, store), {"pages": 2, "attachments": 1})
        self.assertEqual(store.pages["character:ada"]["tags"], ["hero"])
        store.pages["character:ada"]["wikitext"] = "Edited locally"
        store.files[(store.pages["character:ada"]["page_id"], "portrait.png")][
            "data"
        ] = "ffff"
        before = copy.deepcopy((store.pages, store.files))
        self.assertEqual(self.apply(plan, store), {"pages": 0, "attachments": 0})
        self.assertEqual((store.pages, store.files), before)

    def test_lost_page_response_requires_fresh_inventory_without_duplicate(self):
        plan, store = self.plan(), TaggedStore()
        store.fail_after_page = True
        with self.assertRaises(ConnectionError):
            self.apply(plan, store)
        self.apply(plan, store)
        self.assertEqual(len(store.pages), 2)
        self.assertEqual(store.creates, 3)

    def test_lost_file_response_is_not_recreated(self):
        plan, store = self.plan(), TaggedStore()
        store.fail_after_file = True
        with self.assertRaises(ConnectionError):
            self.apply(plan, store)
        self.assertEqual(self.apply(plan, store), {"pages": 0, "attachments": 0})
        self.assertEqual(store.creates, 3)

    def test_limits_do_not_create_files_for_uncreated_owners(self):
        plan, store = self.plan(), TaggedStore()
        self.assertEqual(
            self.apply(plan, store, pages=0, files=10), {"pages": 0, "attachments": 0}
        )
        self.assertEqual(store.creates, 0)
        result = self.apply(plan, store, pages=1, files=0)
        self.assertEqual(result, {"pages": 1, "attachments": 0})

    def test_wrong_endpoint_and_stale_inventory_fail_without_overwrite(self):
        plan, store = self.plan(), TaggedStore()
        snapshot = inventory(plan, store)
        snapshot["rpc_endpoint"] = "http://127.0.0.1:9999/jsonrpc"
        with self.assertRaises(ValueError):
            self.apply(plan, store, snapshot)
        self.assertEqual(store.creates, 0)
        snapshot = inventory(plan, store)
        self.apply(plan, store)
        before = copy.deepcopy((store.pages, store.files))
        with self.assertRaises(poc.PocImportError):
            self.apply(plan, store, snapshot)
        self.assertEqual((store.pages, store.files), before)


if __name__ == "__main__":
    unittest.main()
