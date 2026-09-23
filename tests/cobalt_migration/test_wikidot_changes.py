import unittest

from tools.cobalt_migration.wikidot_changes import parse_changes

DATE = "format_%25e%20%25b%20%25Y%20-%20%25H%3A%25M%3A%25S%7Cagohover"

# Two items captured from cobalt-company.wikidot.com SiteChangesListModule.
BODY = f"""<div class="changes-list-item">
  <table><tr>
    <td class="title"><a href="/writing:2025-09-04-thousand-needles:ice-cream-empire">
      writing:
      (2025-09-04) Thousand Needles: Ice Cream Empire</a></td>
    <td class="flags"><span class="spantip" title="tags changed">A</span></td>
    <td  class="mod-date"><span class="odate time_1790180877 {DATE}">23 Sep 2026 16:27</span></td>
    <td class="revision-no">(rev. 3)</td>
    <td class="mod-by"><span class="printuser"><a href="https://www.wikidot.com/user:info/allicat" onclick="WIKIDOT.page.listeners.userInfo(7570574); return false;" >Allicat</a></span></td>
  </tr></table>
  <div class="comments">
    Added tags: kalimdor-newbies.
  </div>
</div>
<div class="changes-list-item">
  <table><tr>
    <td class="title"><a href="/writing:2026-09-19-a-bold-idea">writing: (2026-09-19) A Bold Idea</a></td>
    <td class="flags"><span class="spantip" title="new page created">N</span></td>
    <td  class="mod-date"><span class="odate time_1790088140 {DATE}">22 Sep 2026 14:42</span></td>
    <td class="revision-no">(new)</td>
    <td class="mod-by"><span class="printuser"><a href="https://www.wikidot.com/user:info/allicat" onclick="WIKIDOT.page.listeners.userInfo(7570574); return false;" >Allicat</a></span></td>
  </tr></table>
</div>"""


class ParseChangesTest(unittest.TestCase):
    def test_reads_revisions_in_listed_order(self):
        self.assertEqual(
            parse_changes(BODY),
            [
                {
                    "slug": "writing:2025-09-04-thousand-needles:ice-cream-empire",
                    "title": "(2025-09-04) Thousand Needles: Ice Cream Empire",
                    "flags": "A",
                    "changed_at": 1790180877,
                    "revision": 3,
                    "user_slug": "allicat",
                    "user_id": 7570574,
                    "user_name": "Allicat",
                    "comments": "Added tags: kalimdor-newbies.",
                },
                {
                    "slug": "writing:2026-09-19-a-bold-idea",
                    "title": "(2026-09-19) A Bold Idea",
                    "flags": "N",
                    "changed_at": 1790088140,
                    "revision": 0,
                    "user_slug": "allicat",
                    "user_id": 7570574,
                    "user_name": "Allicat",
                    "comments": "",
                },
            ],
        )

    def test_empty_page_has_no_revisions(self):
        self.assertEqual(parse_changes('<div class="changes-list"></div>'), [])


if __name__ == "__main__":
    unittest.main()
