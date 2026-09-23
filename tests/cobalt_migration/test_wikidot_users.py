import unittest

from tools.cobalt_migration.wikidot_users import authors, parse_profile

# Excerpt of www.wikidot.com/user:info/allicat.
PROFILE = """<dl>
        <dt>Wikidot user since:</dt> <dd><span class="odate time_1627135892 format_%25e%20%25b%20%25Y%2C%20%25H%3A%25M%20%28%25O%20ago%29">24 Jul 2021 14:11</span></dd>
        <dt>Account type:</dt>
        <dd>
                            free
                    </dd>
        <dt>Karma level:</dt><dd>very high (<a href="#">what is this?</a>)</dd>
</dl>
<script>USERINFO = {}; USERINFO.userId = 7570574;</script>"""


class WikidotUsersTest(unittest.TestCase):
    def test_profile_gives_join_time_karma_and_account_type(self):
        self.assertEqual(
            parse_profile(PROFILE),
            {"user_id": 7570574, "created_at": 1627135892, "karma": 4, "is_pro": False},
        )

    def test_authors_are_distinct_revision_authors(self):
        rows = [
            {"user_id": 7570574, "user_slug": "allicat", "user_name": "Allicat"},
            {"user_id": 7570574, "user_slug": "allicat", "user_name": "Allicat"},
            {"user_id": 7444794, "user_slug": "ozmaasimov", "user_name": "OzmaAsimov"},
        ]
        self.assertEqual(
            authors(rows),
            {7570574: ("allicat", "Allicat"), 7444794: ("ozmaasimov", "OzmaAsimov")},
        )


if __name__ == "__main__":
    unittest.main()
