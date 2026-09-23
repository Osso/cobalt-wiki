"""Synthetic author profiles; no real account names or private profile values."""

import unittest

from tools.cobalt_migration.history_author import (
    HistoryAuthorError,
    parse_history_author,
)


def profile(*, account_date=1700000000, membership_date=1710000000):
    return (
        '<div class="modal-body"><h1>Archive Editor</h1><table>'
        "<tr><td>Wikidot.com User since:</td>"
        f'<td><span class="odate time_{account_date}">localized date</span></td></tr>'
        "<tr><td>Account type</td><td>Pro account</td></tr>"
        "<tr><td>Karma level</td><td><span>Very high</span></td></tr>"
        "<tr><td>Member of this Site: since</td>"
        f'<td><span class="odate time_{membership_date}">localized date</span></td></tr>'
        "<tr><td>Role in this Site</td><td>Member</td></tr>"
        "<tr><td>Real name</td><td>Private Identity</td></tr>"
        "<tr><td>Biography</td><td>Private biography</td></tr>"
        '</table><a href="https://www.wikidot.com/user:info/archive-editor">Profile</a></div>'
    )


class HistoryAuthorTest(unittest.TestCase):
    def test_extracts_sourced_identity_account_date_and_raw_account_fields(self):
        self.assertEqual(
            parse_history_author(profile(), user_id=42, fetched_at=1790000000),
            {
                "source_author_id": 42,
                "display_name": "Archive Editor",
                "slug": "archive-editor",
                "created_at": 1700000000,
                "account_type": "Pro account",
                "karma": "Very high",
                "fetched_at": 1790000000,
            },
        )

    def test_account_creation_is_not_site_membership_date(self):
        record = parse_history_author(
            profile(account_date=1620000000, membership_date=1780000000),
            user_id=7,
            fetched_at=1790000000,
        )
        self.assertEqual(record["created_at"], 1620000000)
        self.assertNotEqual(record["created_at"], 1780000000)

    def test_missing_or_ambiguous_identity_fails(self):
        original = profile()
        for html in (
            original.replace("<h1>Archive Editor</h1>", ""),
            original.replace(
                "<h1>Archive Editor</h1>", "<h1>Archive Editor</h1><h1>Other</h1>"
            ),
            original.replace("/user:info/archive-editor", "/profile/archive-editor"),
            original.replace(
                "</div>",
                '<a href="https://www.wikidot.com/user:info/other">Other</a></div>',
            ),
            original.replace("/user:info/archive-editor", "/user:info/"),
            original.replace(
                "/user:info/archive-editor", "/user:info/archive-editor?from=wiki"
            ),
        ):
            with (
                self.subTest(html_length=len(html)),
                self.assertRaises(HistoryAuthorError),
            ):
                parse_history_author(html, user_id=42, fetched_at=1790000000)

    def test_missing_or_ambiguous_account_date_fails_without_using_membership(self):
        original = profile()
        for html in (
            original.replace('class="odate time_1700000000"', 'class="odate"'),
            original.replace('class="odate time_1700000000"', 'class="odate time_0"'),
            original.replace(
                'class="odate time_1700000000"', 'class="odate time_nope"'
            ),
            original.replace(
                '<span class="odate time_1700000000">localized date</span>', ""
            ),
            original.replace("time_1700000000", "time_1700000000 time_1700000001"),
            original.replace("Wikidot.com User since:", "Member of this Site: since"),
        ):
            with (
                self.subTest(html_length=len(html)),
                self.assertRaises(HistoryAuthorError),
            ):
                parse_history_author(html, user_id=42, fetched_at=1790000000)

    def test_account_type_karma_and_request_identity_are_required(self):
        original = profile()
        for html in (
            original.replace("Pro account", ""),
            original.replace("Very high", ""),
            original.replace("Karma level", "Account type"),
        ):
            with (
                self.subTest(html_length=len(html)),
                self.assertRaises(HistoryAuthorError),
            ):
                parse_history_author(html, user_id=42, fetched_at=1790000000)
        for user_id, fetched_at in (
            (0, 1790000000),
            (True, 1790000000),
            (42, 0),
            (42, True),
        ):
            with (
                self.subTest(user_id=user_id, fetched_at=fetched_at),
                self.assertRaises(HistoryAuthorError),
            ):
                parse_history_author(original, user_id=user_id, fetched_at=fetched_at)


if __name__ == "__main__":
    unittest.main()
