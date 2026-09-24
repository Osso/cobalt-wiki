"""Behavioral tests for conservative, read-only missing import selection."""

import unittest

from tools.cobalt_migration.poc_missing_inventory import select_missing


def page(fullname):
    return {
        "fullname": fullname,
        "path": f"source/{fullname.replace(':', '_')}.txt",
        "sha256": "a" * 64,
        "size": 5,
        "title": fullname,
        "tags": [],
    }


def attachment(fullname, name):
    return {
        "fullname": fullname,
        "name": name,
        "path": f"files/{fullname.replace(':', '_')}/{name}",
        "sha256": "b" * 64,
        "size": 3,
    }


def plan(pages, attachments):
    return {
        "schema": 1,
        "site_id": 6000000,
        "plan_sha256": "c" * 64,
        "pages": pages,
        "attachments": attachments,
    }


def inventory():
    return {
        "schema": 1,
        "site_id": 6000000,
        "plan_sha256": "c" * 64,
        "rpc_endpoint": "https://local.invalid/rpc",
        "pages": [],
        "page_revisions": [],
        "files": [],
        "file_revisions": [],
        "orphan_audit_page_ids": [],
    }


def current_page(page_id, slug, deleted=False):
    return {
        "page_id": page_id,
        "slug": slug,
        "deleted": deleted,
        "latest_revision_id": page_id + 100,
    }


def current_file(file_id, page_id, name, deleted=False):
    return {"file_id": file_id, "page_id": page_id, "name": name, "deleted": deleted}


class MissingInventoryTests(unittest.TestCase):
    def test_existing_edited_page_and_file_are_preserved_without_content_matching(self):
        source_page = page("home:start")
        source_file = attachment("home:start", "banner.png")
        target = inventory()
        target["pages"] = [current_page(21, "home:start")]
        target["page_revisions"] = [
            {
                "page_id": 21,
                "slug": "home:start",
                "revision_id": 101,
                "content": "edited",
            }
        ]
        target["files"] = [current_file(31, 21, "banner.png")]
        target["file_revisions"] = [
            {"file_id": 31, "page_id": 21, "name": "banner.png", "sha256": "edited"}
        ]
        self.assertEqual(
            select_missing(plan([source_page], [source_file]), target),
            {
                "pages": [],
                "attachments": [],
                "existing_pages": {"home:start": 21},
                "skipped": {"present_page": 1, "present_file": 1},
            },
        )

    def test_deleted_and_moved_names_are_retired_not_recreated(self):
        target = inventory()
        target["pages"] = [
            current_page(2, "old:deleted", True),
            current_page(3, "new:address"),
        ]
        target["page_revisions"] = [
            {"page_id": 3, "slug": "old:address", "revision_id": 77}
        ]
        result = select_missing(
            plan([page("old:deleted"), page("old:address")], []), target
        )
        self.assertEqual(result["pages"], [])
        self.assertEqual(result["skipped"], {"retired_page": 2})
        self.assertEqual(result["existing_pages"], {})

    def test_new_page_and_its_files_and_missing_file_on_active_owner_selected(self):
        new = page("new:article")
        existing = page("old:article")
        fresh = attachment("new:article", "cover.jpg")
        additional = attachment("old:article", "cover.jpg")
        target = inventory()
        target["pages"] = [current_page(9, "old:article")]
        result = select_missing(plan([new, existing], [fresh, additional]), target)
        self.assertEqual(result["pages"], [new])
        self.assertEqual(result["attachments"], [fresh, additional])
        self.assertEqual(result["existing_pages"], {"old:article": 9})
        self.assertEqual(result["skipped"], {"present_page": 1})

    def test_historical_file_name_and_deleted_file_block_recreation(self):
        source = page("home:start")
        old_name = attachment("home:start", "old.png")
        deleted = attachment("home:start", "deleted.png")
        target = inventory()
        target["pages"] = [current_page(7, "home:start")]
        target["files"] = [
            current_file(40, 7, "new.png"),
            current_file(41, 7, "deleted.png", True),
        ]
        target["file_revisions"] = [
            {"file_id": 40, "page_id": 7, "name": "old.png", "revision_id": 99}
        ]
        result = select_missing(plan([source], [old_name, deleted]), target)
        self.assertEqual(result["attachments"], [])
        self.assertEqual(result["skipped"], {"present_page": 1, "present_file": 2})

    def test_replacement_page_with_same_historical_name_is_ambiguous_for_files(self):
        target = inventory()
        target["pages"] = [
            current_page(11, "home:start", True),
            current_page(12, "home:start"),
        ]
        result = select_missing(
            plan([page("home:start")], [attachment("home:start", "new.png")]),
            target,
        )
        self.assertEqual(result["pages"], [])
        self.assertEqual(result["attachments"], [])
        self.assertEqual(result["existing_pages"], {"home:start": 12})
        self.assertEqual(result["skipped"], {"present_page": 1, "ambiguous_owner": 1})

    def test_multiple_active_owners_skip_ambiguous_attachment(self):
        target = inventory()
        target["pages"] = [
            current_page(12, "home:start"),
            current_page(13, "home:start"),
        ]
        result = select_missing(
            plan([page("home:start")], [attachment("home:start", "new.png")]),
            target,
        )
        self.assertEqual(result["existing_pages"], {})
        self.assertEqual(result["attachments"], [])
        self.assertEqual(result["skipped"], {"present_page": 1, "ambiguous_owner": 1})

    def test_historical_replacement_owner_blocks_attachment(self):
        target = inventory()
        target["pages"] = [current_page(12, "home:start")]
        target["page_revisions"] = [
            {"page_id": 11, "slug": "home:start", "revision_id": 30}
        ]
        result = select_missing(
            plan([page("home:start")], [attachment("home:start", "new.png")]),
            target,
        )
        self.assertEqual(result["existing_pages"], {"home:start": 12})
        self.assertEqual(result["attachments"], [])
        self.assertEqual(result["skipped"], {"present_page": 1, "ambiguous_owner": 1})

    def test_missing_file_on_retired_or_deleted_owner_is_not_selected(self):
        target = inventory()
        target["pages"] = [
            current_page(10, "elsewhere:now"),
            current_page(11, "deleted:page", True),
        ]
        target["page_revisions"] = [{"page_id": 10, "slug": "former:page"}]
        result = select_missing(
            plan(
                [page("former:page"), page("deleted:page")],
                [attachment("former:page", "a"), attachment("deleted:page", "b")],
            ),
            target,
        )
        self.assertEqual(result["attachments"], [])
        self.assertEqual(result["skipped"], {"retired_page": 2, "unavailable_owner": 2})

    def test_scope_and_incomplete_or_ambiguous_inventory_fail_closed(self):
        source = plan([page("home:start")], [])
        changes = [
            ("site_id", 8),
            ("plan_sha256", "wrong"),
            ("schema", 2),
            ("rpc_endpoint", ""),
            ("page_revisions", None),
            ("files", None),
            ("orphan_audit_page_ids", [21]),
            ("pages", [current_page(1, "a"), current_page(1, "b")]),
            ("files", [current_file(1, 10, "a"), current_file(1, 10, "b")]),
        ]
        for key, value in changes:
            with self.subTest(key=key, value=value):
                target = inventory()
                target[key] = value
                with self.assertRaises(ValueError):
                    select_missing(source, target)

    def test_conflicting_revision_ids_fail_closed(self):
        for key, first, second in [
            (
                "page_revisions",
                {"page_id": 1, "slug": "one", "revision_id": 7},
                {"page_id": 1, "slug": "two", "revision_id": 7},
            ),
            (
                "file_revisions",
                {"file_id": 2, "page_id": 1, "name": "one", "revision_id": 8},
                {"file_id": 2, "page_id": 1, "name": "two", "revision_id": 8},
            ),
        ]:
            with self.subTest(key=key):
                target = inventory()
                target[key] = [first, second]
                with self.assertRaises(ValueError):
                    select_missing(plan([page("one")], []), target)

    def test_malformed_records_fail_instead_of_appearing_absent(self):
        for key, record in [
            ("pages", {"page_id": 1, "slug": "x"}),
            ("page_revisions", {"page_id": 1}),
            ("files", {"file_id": 1, "page_id": 1, "name": "x"}),
            ("file_revisions", {"file_id": 1, "page_id": 1}),
        ]:
            with self.subTest(key=key):
                target = inventory()
                target[key] = [record]
                with self.assertRaises(ValueError):
                    select_missing(plan([page("x")], []), target)


if __name__ == "__main__":
    unittest.main()
