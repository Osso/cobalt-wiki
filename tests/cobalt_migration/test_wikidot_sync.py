import unittest

from tools.cobalt_migration.wikidot_sync import apply_sql, decode_view_source, parse_meta

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


if __name__ == "__main__":
    unittest.main()
