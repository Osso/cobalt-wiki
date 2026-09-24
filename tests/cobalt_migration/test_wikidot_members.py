import unittest

from tools.cobalt_migration.wikidot_members import import_member

SITE = 6000000
IMPORTER = -1
ROLE_IDS = {"root": 1, "admin": 2, "moderator": 3, "member": 4}
OZMA = {"name": "OzmaAsimov", "slug": "ozmaasimov", "user_id": 7444794,
        "member_since_epoch": 1619708100, "role": "admin", "master_admin": True}
LURIDEL = {"name": "Luridel", "slug": "luridel", "user_id": 7512345,
           "member_since_epoch": 1630070160, "role": "moderator", "master_admin": False}


class FakeDeepwell:
    """Answers the read RPCs from fixed state and records every call."""

    def __init__(self, user=None, member=None, roles=()):
        self.user, self.member, self.roles = user, member, list(roles)
        self.calls = []

    def rpc(self, method, params):
        self.calls.append((method, params))
        return {
            "user_get": self.user,
            "member_get": self.member,
            "user_role_list": [{"name": name} for name in self.roles],
        }.get(method, {})

    def methods(self):
        return [method for method, _ in self.calls]

    def params(self, method):
        return [params for called, params in self.calls if called == method]


def profile(user_id):
    return lambda slug: {"user_id": user_id, "created_at": 1604390400, "karma": 3, "is_pro": False}


class ImportMemberTest(unittest.TestCase):
    def test_unknown_member_gets_record_membership_roles_then_account(self):
        rpc = FakeDeepwell()
        self.assertEqual(import_member(rpc, OZMA, SITE, ROLE_IDS, IMPORTER, profile(7444794)), "created")
        self.assertEqual(rpc.methods(), [
            "user_get", "import_wikidot_user", "member_get", "member_set", "user_role_list",
            "user_role_grant", "user_role_grant", "user_role_grant", "user_activate_from_wikidot",
        ])
        record = rpc.params("import_wikidot_user")[0]
        self.assertEqual((record["user_id"], record["name"], record["slug"], record["created_at"], record["karma"]),
                         (7444794, "OzmaAsimov", "ozmaasimov", "2020-11-03T08:00:00Z", 3))
        self.assertEqual(rpc.params("member_set")[0]["joined_at"], "2021-04-29T14:55:00Z")
        self.assertEqual(sorted(g["role_id"] for g in rpc.params("user_role_grant")), [1, 2, 4])
        account = rpc.params("user_activate_from_wikidot")[0]
        self.assertEqual(account["user_id"], 7444794)
        self.assertNotIn("created_at", account)
        self.assertEqual(account["email"], "wikidot-7444794@members.invalid")
        self.assertGreaterEqual(len(account["password"]), 64)

    def test_each_account_gets_a_different_password(self):
        passwords = set()
        for _ in range(2):
            rpc = FakeDeepwell(user={"user_type": "wikidot", "slug": "luridel"})
            import_member(rpc, LURIDEL, SITE, ROLE_IDS, IMPORTER)
            passwords.add(rpc.params("user_activate_from_wikidot")[0]["password"])
        self.assertEqual(len(passwords), 2)

    def test_existing_account_is_left_untouched(self):
        rpc = FakeDeepwell(user={"user_type": "regular", "slug": "ozmaasimov"})
        self.assertEqual(import_member(rpc, OZMA, SITE, ROLE_IDS, IMPORTER), "exists")
        self.assertEqual(rpc.methods(), ["user_get"])

    def test_rerun_after_partial_import_adds_only_what_is_missing(self):
        rpc = FakeDeepwell(user={"user_type": "wikidot", "slug": "luridel"},
                           member={"relation_id": 9}, roles=["member", "registered"])
        self.assertEqual(import_member(rpc, LURIDEL, SITE, ROLE_IDS, IMPORTER), "created")
        self.assertEqual(rpc.methods(), [
            "user_get", "member_get", "user_role_list", "user_role_grant", "user_activate_from_wikidot",
        ])
        self.assertEqual(rpc.params("user_role_grant")[0]["role_id"], 3)

    def test_profile_of_another_user_is_refused(self):
        rpc = FakeDeepwell()
        with self.assertRaises(ValueError):
            import_member(rpc, OZMA, SITE, ROLE_IDS, IMPORTER, profile(1))
        self.assertEqual(rpc.methods(), ["user_get"])

    def test_wikidot_record_with_other_slug_is_refused(self):
        rpc = FakeDeepwell(user={"user_type": "wikidot", "slug": "someone-else"})
        with self.assertRaises(ValueError):
            import_member(rpc, LURIDEL, SITE, ROLE_IDS, IMPORTER)
        self.assertEqual(rpc.methods(), ["user_get"])


if __name__ == "__main__":
    unittest.main()
