import unittest

from tools.cobalt_migration.wikidot_dates import parse_dates

FORMAT = "format_%25e%20%25b%20%25Y%2C%20%25H%3A%25M%7Cagohover"

# Captured from cobalt-company.wikidot.com with perPage=2, p=3.
BODY = f"""<div class="list-pages-box">

<p><span class="date-row">search:site|<span class="odate time_1619726117 {FORMAT}">29 Apr 2021 19:55</span>|<span class="odate time_1619726117 {FORMAT}">29 Apr 2021 19:55</span></span><br />
<span class="date-row">home:start|<span class="odate time_1619726117 {FORMAT}">29 Apr 2021 19:55</span>|<span class="odate time_1780938694 {FORMAT}">08 Jun 2026 17:11</span></span></p>

    <div class="pager"><span class="pager-no">page 3 of 3041</span><span class="target"><a href="/ajax-module-connector.php/p/2">&laquo; previous</a></span></div>
</div>"""


class ParseDatesTest(unittest.TestCase):
    def test_reads_fullnames_dates_and_page_count(self):
        self.assertEqual(
            parse_dates(BODY),
            (
                {
                    "search:site": {"created_at": 1619726117, "updated_at": 1619726117},
                    "home:start": {"created_at": 1619726117, "updated_at": 1780938694},
                },
                3041,
            ),
        )

    def test_single_page_listing_has_no_pager(self):
        rows, pages = parse_dates(BODY.split('<div class="pager">')[0])
        self.assertEqual((len(rows), pages), (2, 1))


if __name__ == "__main__":
    unittest.main()
