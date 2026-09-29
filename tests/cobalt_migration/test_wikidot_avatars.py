import unittest

from tools.cobalt_migration.wikidot_avatars import MAX_AVATAR_BYTES, run

PNG = b"\x89PNG\r\n\x1a\n" + b"x" * 100
CUSTOM = "https://d2qhngyckgiutd.cloudfront.net/a200d467c6813c24872b3d2728d618a4"
DEFAULT = "https://d2qhngyckgiutd.cloudfront.net/default_avatar"


class FakeDeepwell:
    """Site members and their current avatar hashes; records every call."""

    def __init__(self, members):
        self.members = members  # (user_id, slug, avatar_s3_hash)
        self.calls = []
        self.puts = []

    def rpc(self, method, params):
        self.calls.append((method, params))
        if method == "member_admin_list":
            return [{"user_id": uid, "slug": slug, "name": slug} for uid, slug, _ in self.members]
        if method == "user_get":
            return next({"user_id": uid, "avatar_s3_hash": avatar}
                        for uid, _, avatar in self.members if uid == params["user"])
        if method == "blob_upload":
            return {"pending_blob_id": f"pending-{params['user_id']}",
                    "presign_url": f"http://127.0.0.1:29000/upload/{params['user_id']}"}
        return {}

    def put(self, url, data):
        self.puts.append((url, data))

    def methods(self):
        return [method for method, _ in self.calls]

    def params(self, method):
        return [params for called, params in self.calls if called == method]


class FakeWikidot:
    """Wikidot by slug (profile user ID) and by user ID (avatar response)."""

    def __init__(self, avatars, ids):
        self.avatars = avatars  # user_id -> (final URL, content type, bytes), or None
        self.ids = ids  # slug -> Wikidot user ID, or None when no such account
        self.fetched = []

    def user_id(self, slug):
        return self.ids[slug]

    def avatar(self, user_id):
        self.fetched.append(user_id)
        return self.avatars[user_id]


def avatars(rpc, wikidot, write=True):
    sleeps = []
    lines = list(run(rpc, wikidot, write, 1.5, sleeps.append))
    return lines, sleeps


class RunTest(unittest.TestCase):
    def test_apply_stores_a_custom_wikidot_avatar_as_the_members_own_upload(self):
        rpc = FakeDeepwell([(121356, "luridel", None)])
        wikidot = FakeWikidot({121356: (CUSTOM, "image/png", PNG)}, {"luridel": 121356})
        lines, sleeps = avatars(rpc, wikidot)
        self.assertEqual(rpc.methods(), ["member_admin_list", "user_get", "blob_upload", "user_edit"])
        self.assertEqual(rpc.params("blob_upload"), [{"user_id": 121356, "blob_size": len(PNG)}])
        self.assertEqual(rpc.puts, [("http://127.0.0.1:29000/upload/121356", PNG)])
        self.assertEqual(rpc.params("user_edit"), [{
            "user": 121356, "avatar_uploaded_blob_id": "pending-121356", "ip_address": "127.0.0.1",
        }])
        self.assertEqual(lines, [f"luridel: import Wikidot avatar ({len(PNG)} bytes, image/png)"])
        self.assertEqual(sleeps, [1.5])

    def test_dry_run_fetches_and_reports_but_writes_nothing(self):
        rpc = FakeDeepwell([(121356, "luridel", None)])
        lines, sleeps = avatars(rpc, FakeWikidot({121356: (CUSTOM, "image/png", PNG)}, {"luridel": 121356}), write=False)
        self.assertEqual(rpc.methods(), ["member_admin_list", "user_get"])
        self.assertEqual(rpc.puts, [])
        self.assertEqual(lines, [f"would luridel: import Wikidot avatar ({len(PNG)} bytes, image/png)"])
        self.assertEqual(sleeps, [])

    def test_wikidot_default_avatar_is_not_imported(self):
        rpc = FakeDeepwell([(7444794, "ozmaasimov", None)])
        lines, _ = avatars(rpc, FakeWikidot({7444794: (DEFAULT, "image/png", PNG)}, {"ozmaasimov": 7444794}))
        self.assertNotIn("blob_upload", rpc.methods())
        self.assertEqual(lines, ["ozmaasimov: Wikidot default avatar, skipped"])

    def test_members_with_an_avatar_or_no_wikidot_id_are_not_fetched(self):
        rpc = FakeDeepwell([(-1, "cobalt-import", None), (354294, "booriley22", [1, 2, 3])])
        wikidot = FakeWikidot({}, {})
        lines, sleeps = avatars(rpc, wikidot)
        self.assertEqual(wikidot.fetched, [])
        self.assertEqual(rpc.params("user_get"), [{"user": 354294}])
        self.assertEqual(lines, ["booriley22: already has an avatar, skipped",
                                 "cobalt-import: no Wikidot account, skipped"])
        self.assertEqual(sleeps, [])

    def test_non_images_and_oversized_images_are_skipped(self):
        rpc = FakeDeepwell([(1, "a-member", None), (2, "b-member", None)])
        wikidot = FakeWikidot({
            1: (CUSTOM, "text/html", b"<html>"),
            2: (CUSTOM, "image/png", b"x" * (MAX_AVATAR_BYTES + 1)),
        }, {"a-member": 1, "b-member": 2})
        lines, _ = avatars(rpc, wikidot)
        self.assertNotIn("blob_upload", rpc.methods())
        self.assertEqual(lines, [
            "a-member: Wikidot returned text/html, skipped",
            f"b-member: Wikidot avatar is {MAX_AVATAR_BYTES + 1} bytes, over the limit, skipped",
        ])

    def test_members_are_processed_in_slug_order(self):
        rpc = FakeDeepwell([(2, "zed", None), (1, "amy", None)])
        wikidot = FakeWikidot({1: (DEFAULT, "image/png", PNG), 2: (DEFAULT, "image/png", PNG)},
                              {"amy": 1, "zed": 2})
        lines, _ = avatars(rpc, wikidot, write=False)
        self.assertEqual([line.split(":")[0] for line in lines], ["would amy", "would zed"])

    def test_local_accounts_whose_id_is_not_their_wikidot_id_are_not_imported(self):
        # A locally created account can share its ID with an unrelated Wikidot user.
        rpc = FakeDeepwell([(20000001, "new-member", None), (7661816, "renamed", None)])
        wikidot = FakeWikidot({20000001: (CUSTOM, "image/png", PNG), 7661816: (CUSTOM, "image/png", PNG)},
                              {"new-member": None, "renamed": 42})
        lines, _ = avatars(rpc, wikidot)
        self.assertEqual(wikidot.fetched, [])
        self.assertNotIn("blob_upload", rpc.methods())
        self.assertEqual(lines, ["new-member: no Wikidot account with this slug, skipped",
                                 "renamed: Wikidot account 42 does not match user 7661816, skipped"])

    def test_missing_wikidot_avatar_is_skipped(self):
        rpc = FakeDeepwell([(5, "gone", None)])
        lines, _ = avatars(rpc, FakeWikidot({5: None}, {"gone": 5}))
        self.assertEqual(lines, ["gone: no Wikidot avatar, skipped"])


if __name__ == "__main__":
    unittest.main()
