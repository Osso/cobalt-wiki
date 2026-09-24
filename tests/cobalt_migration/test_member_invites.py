import os
import stat
import tempfile
import unittest

from tools.cobalt_migration.member_invites import load_invites, mask, run, write_links

SITE = 6000000
PLACEHOLDER = "wikidot-7444794@members.invalid"
NO_LINK = {"password_chosen": False, "pending": None}


def pending(emailed):
    return {"password_chosen": False, "pending": {"expires_at": "2026-10-01T00:00:00Z", "emailed": emailed}}


class FakeDeepwell:
    """Answers the read RPCs from fixed per-slug state and records every call."""

    def __init__(self, users):
        self.users = users  # slug -> (email, status)
        self.calls = []

    def rpc(self, method, params):
        self.calls.append((method, params))
        entry = self.users.get(params["user"])
        if method == "user_get":
            return entry and {"user_type": "regular", "slug": params["user"], "email": entry[0]}
        if method == "password_token_status":
            return entry[1]
        if method == "password_token_create":
            return {"path": f"/-/set-password/secret-{params['user']}", "emailed": params["send_email"]}
        return {}

    def methods(self):
        return [method for method, _ in self.calls]

    def params(self, method):
        return [params for called, params in self.calls if called == method]


def invite(rpc, invites, send=False, write=True):
    links, sleeps = {}, []
    lines = list(run(rpc, invites, SITE, send, write, links, 2.0, sleeps.append))
    return lines, links, sleeps


class RunTest(unittest.TestCase):
    def test_send_sets_email_without_verification_then_emails_a_link(self):
        rpc = FakeDeepwell({"ozmaasimov": (PLACEHOLDER, NO_LINK)})
        lines, links, sleeps = invite(rpc, {"ozmaasimov": "ozma@example.com"}, send=True)
        self.assertEqual(rpc.methods(), ["user_get", "password_token_status", "user_edit", "password_token_create"])
        self.assertEqual(rpc.params("user_edit"), [{
            "user": "ozmaasimov", "email": "ozma@example.com",
            "bypass_filter": True, "bypass_email_verification": True, "ip_address": "127.0.0.1",
        }])
        self.assertEqual(rpc.params("password_token_create"),
                         [{"user": "ozmaasimov", "site_id": SITE, "send_email": True}])
        self.assertEqual(lines, ["ozmaasimov: email w***@members.invalid -> o***@example.com; "
                                 "email a set-password link to o***@example.com"])
        self.assertEqual(links, {}, "emailed links are not kept")
        self.assertEqual(sleeps, [2.0])

    def test_dry_run_only_reads_and_masks_every_address(self):
        rpc = FakeDeepwell({"ozmaasimov": (PLACEHOLDER, NO_LINK), "luridel": (PLACEHOLDER, NO_LINK)})
        lines, _, sleeps = invite(rpc, {"ozmaasimov": "ozma@example.com", "luridel": "luri@example.org"},
                                  send=True, write=False)
        self.assertEqual(set(rpc.methods()), {"user_get", "password_token_status"})
        self.assertEqual(lines, [
            "would luridel: email w***@members.invalid -> l***@example.org; "
            "email a set-password link to l***@example.org",
            "would ozmaasimov: email w***@members.invalid -> o***@example.com; "
            "email a set-password link to o***@example.com",
        ])
        self.assertNotIn("ozma@example.com", "\n".join(lines))
        self.assertEqual(sleeps, [])

    def test_rerun_skips_members_done_or_already_emailed(self):
        rpc = FakeDeepwell({
            "ozmaasimov": ("ozma@example.com", pending(emailed=True)),
            "luridel": ("luri@example.org", {"password_chosen": True, "pending": None}),
        })
        lines, _, sleeps = invite(rpc, {"ozmaasimov": "ozma@example.com", "luridel": "luri@example.org"},
                                  send=True)
        self.assertEqual(set(rpc.methods()), {"user_get", "password_token_status"})
        self.assertEqual(lines, [
            "luridel: password already chosen",
            "ozmaasimov: link pending until 2026-10-01T00:00:00Z (emailed)",
        ])
        self.assertEqual(sleeps, [])

    def test_unemailed_link_is_replaced_when_sending_but_kept_otherwise(self):
        users = {"ozmaasimov": ("ozma@example.com", pending(emailed=False))}
        rpc = FakeDeepwell(users)
        invite(rpc, {"ozmaasimov": "ozma@example.com"}, send=False)
        self.assertNotIn("password_token_create", rpc.methods())

        rpc = FakeDeepwell(users)
        invite(rpc, {"ozmaasimov": "ozma@example.com"}, send=True)
        self.assertEqual(rpc.params("password_token_create")[0]["send_email"], True)

    def test_changed_email_gets_a_new_link_even_with_one_pending(self):
        rpc = FakeDeepwell({"ozmaasimov": ("old@example.com", pending(emailed=True))})
        invite(rpc, {"ozmaasimov": "ozma@example.com"}, send=True)
        self.assertEqual(rpc.methods()[2:], ["user_edit", "password_token_create"])

    def test_apply_keeps_links_for_the_file_and_never_prints_them(self):
        rpc = FakeDeepwell({"ozmaasimov": ("ozma@example.com", NO_LINK)})
        lines, links, _ = invite(rpc, {"ozmaasimov": "ozma@example.com"}, send=False)
        self.assertEqual(rpc.params("password_token_create")[0]["send_email"], False)
        self.assertEqual(links, {"ozmaasimov": "/-/set-password/secret-ozmaasimov"})
        self.assertEqual(lines, ["ozmaasimov: create a set-password link"])

        with tempfile.TemporaryDirectory() as directory:
            path = os.path.join(directory, "links.tsv")
            write_links(path, links)
            self.assertEqual(stat.S_IMODE(os.stat(path).st_mode), 0o600)
            with open(path) as file:
                self.assertEqual(file.read(), "ozmaasimov\t/-/set-password/secret-ozmaasimov\n")
            with self.assertRaises(FileExistsError):
                write_links(path, links)

    def test_member_without_account_stops_the_run(self):
        rpc = FakeDeepwell({})
        with self.assertRaisesRegex(ValueError, "ghost: no regular account"):
            invite(rpc, {"ghost": "ghost@example.com"}, send=True)
        self.assertEqual(rpc.methods(), ["user_get"])


class LoadInvitesTest(unittest.TestCase):
    def write(self, name, text, mode=0o600):
        path = os.path.join(self.directory.name, name)
        with open(path, "w") as file:
            file.write(text)
        os.chmod(path, mode)
        return path

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()

    def tearDown(self):
        self.directory.cleanup()

    def test_csv_and_json_give_the_same_mapping(self):
        expected = {"ozmaasimov": "ozma@example.com", "luridel": "luri@example.org"}
        csv_path = self.write("invites.csv", "slug,email\nozmaasimov, ozma@example.com\nluridel,luri@example.org\n")
        json_path = self.write("invites.json", '{"ozmaasimov": "ozma@example.com", "luridel": "luri@example.org"}')
        self.assertEqual(load_invites(csv_path), expected)
        self.assertEqual(load_invites(json_path), expected)

    def test_readable_file_is_refused(self):
        path = self.write("invites.csv", "slug,email\nozmaasimov,ozma@example.com\n", mode=0o644)
        with self.assertRaisesRegex(ValueError, "chmod 600"):
            load_invites(path)

    def test_bad_rows_are_refused(self):
        for text, message in [
            ("slug,email\nozmaasimov,not-an-address\n", "not a deliverable"),
            (f"slug,email\nozmaasimov,{PLACEHOLDER}\n", "not a deliverable"),
            ("slug,email\na,x@example.com\na,y@example.com\n", "listed twice"),
            ("slug,email\na,x@example.com\nb,X@example.com\n", "share an email"),
        ]:
            with self.assertRaisesRegex(ValueError, message):
                load_invites(self.write("invites.csv", text))

    def test_mask_keeps_first_letter_and_domain(self):
        self.assertEqual(mask("ozma@example.com"), "o***@example.com")


if __name__ == "__main__":
    unittest.main()
