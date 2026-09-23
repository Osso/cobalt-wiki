"""Synthetic Wikidot history lists; no source prose or author names."""

import unittest

from tools.cobalt_migration.page_history import (
    PageHistoryError,
    parse_history_list,
    parse_history_pages,
)


def revision(number, revision_id, *, author_id=None, epoch=None, comment="", flag="S"):
    author = (
        f'<span class="printuser"><a onclick="WIKIDOT.page.listeners.userInfo({author_id}); return false;">Account</a>'
        f'<a onclick="WIKIDOT.page.listeners.userInfo({author_id}); return false;">Account</a></span>'
        if author_id is not None
        else '<span class="printuser">Unknown</span>'
    )
    date = (
        f'<span class="odate time_{epoch}">localized date</span>'
        if epoch is not None
        else ""
    )
    return (
        f'<tr id="revision-row-{revision_id}"><td>{number}.</td><td>selection</td>'
        f'<td><span class="spantip" title="source text changed">{flag}</span></td>'
        f'<td class="optionstd"><a onclick="showVersion({revision_id})">View</a>'
        f'<a onclick="showSource({revision_id})">Source</a></td><td>{author}</td>'
        f'<td>{date}<td style="font-size: 90%">{comment}</td></td></tr>'
    )


def history(rows, *, page=1, next_page=None, previous_page=None):
    targets = []
    if previous_page is not None:
        targets.append((previous_page, "« previous"))
    if next_page is not None:
        targets.append((next_page, "next »"))
    links = "".join(
        f'<a onclick="updatePagedList({target})">{label}</a>'
        for target, label in targets
    )
    return (
        f'<div class="pager"><span class="pager-no">page</span>'
        f'<span class="current">{page}</span>{links}</div>'
        f'<div class="page-history"><table><tr><th>revision</th></tr>{"".join(rows)}</table></div>'
    )


class PageHistoryTest(unittest.TestCase):
    def test_history_on_one_list_page_has_no_pager(self):
        html = history([revision(1, 10001), revision(0, 10000, flag="N")])
        result = parse_history_list(html[html.index('<div class="page-history">'):])
        self.assertEqual(
            (result["page"], result["previous_page"], result["next_page"]), (1, None, None)
        )
        self.assertEqual([row["number"] for row in result["revisions"]], [1, 0])

    def test_sourced_revision_metadata_and_pager(self):
        result = parse_history_list(
            history(
                [
                    revision(
                        21,
                        10021,
                        author_id=7,
                        epoch=1780938694,
                        comment="Edited &amp; reviewed",
                    ),
                    revision(20, 10020, comment=""),
                ],
                page=1,
                next_page=2,
            )
        )
        self.assertEqual(result["page"], 1)
        self.assertEqual(result["next_page"], 2)
        self.assertIsNone(result["previous_page"])
        self.assertEqual(
            result["revisions"],
            [
                {
                    "number": 21,
                    "revision_id": 10021,
                    "author_id": 7,
                    "created_at": 1780938694,
                    "comments": "Edited & reviewed",
                    "flags": [{"marker": "S", "description": "source text changed"}],
                },
                {
                    "number": 20,
                    "revision_id": 10020,
                    "author_id": None,
                    "created_at": None,
                    "comments": None,
                    "flags": [{"marker": "S", "description": "source text changed"}],
                },
            ],
        )

    def test_absent_author_and_date_are_recorded_as_unknown(self):
        html = history(
            [revision(0, 100).replace('<span class="printuser">Unknown</span>', "")]
        )
        record = parse_history_list(html)["revisions"][0]
        self.assertIsNone(record["author_id"])
        self.assertIsNone(record["created_at"])

    def test_native_td_header_is_not_a_revision(self):
        html = history([revision(0, 100)])
        html = html.replace(
            "<tr><th>revision</th></tr>", "<tr><td>rev.</td><td>type</td></tr>"
        )
        self.assertEqual(
            [item["number"] for item in parse_history_list(html)["revisions"]], [0]
        )

    def test_multiple_pages_validate_order_without_inventing_missing_revisions(self):
        pages = [
            history([revision(9, 900), revision(7, 700)], next_page=2),
            history([revision(6, 600), revision(0, 100)], page=2, previous_page=1),
        ]
        result = parse_history_pages(pages)
        self.assertEqual(
            [record["number"] for record in result["revisions"]], [9, 7, 6, 0]
        )
        self.assertEqual(result["unobserved_ranges"], [(8, 8), (5, 1)])

    def test_unobserved_initial_revision_is_reported_as_gap_not_fabricated(self):
        result = parse_history_pages([history([revision(2, 900), revision(1, 800)])])
        self.assertEqual([row["number"] for row in result["revisions"]], [2, 1])
        self.assertEqual(result["unobserved_ranges"], [(0, 0)])

    def test_duplicate_revision_numbers_and_global_ids_fail_across_pages(self):
        for rows in (
            [revision(2, 900), revision(2, 901)],
            [revision(2, 900), revision(1, 900)],
        ):
            with self.subTest(rows=len(rows)), self.assertRaises(PageHistoryError):
                parse_history_list(history(rows))
        first = history([revision(2, 900)], next_page=2)
        for second in (revision(2, 901), revision(1, 900)):
            with self.subTest(second=second), self.assertRaises(PageHistoryError):
                parse_history_pages([first, history([second], page=2, previous_page=1)])

    def test_conflicting_or_missing_source_ids_fail_explicitly(self):
        valid = revision(1, 800)
        for invalid in (
            valid.replace("showSource(800)", "showSource(801)"),
            valid.replace("showSource(800)", "showSource(nope)"),
            valid.replace('id="revision-row-800"', 'id="revision-row-0"'),
            valid.replace(
                "showSource(800)", 'showSource(800)</a><a onclick="showSource(900)"'
            ),
        ):
            with (
                self.subTest(invalid=invalid[:45]),
                self.assertRaises(PageHistoryError),
            ):
                parse_history_list(history([invalid]))

    def test_bad_revision_numbers_dates_authors_and_pager_fail(self):
        valid = revision(1, 800, author_id=7, epoch=100)
        for invalid in (
            valid.replace("<td>1.</td>", "<td>not-number.</td>"),
            valid.replace("time_100", "time_invalid"),
            valid.replace("time_100", "time_0"),
            valid.replace("userInfo(7)", "userInfo(8)", 1),
            valid.replace("showVersion(800)", "showVersion(801)"),
        ):
            with (
                self.subTest(invalid=invalid[:45]),
                self.assertRaises(PageHistoryError),
            ):
                parse_history_list(history([invalid]))
        with self.assertRaises(PageHistoryError):
            parse_history_list(history([valid], page=2, previous_page=4))

    def test_denied_empty_and_sparse_lists_fail_without_fabricating_records(self):
        for html in (
            "Permission denied",
            history([]),
            '<div class="page-history">Not available</div>',
        ):
            with self.subTest(html=html[:25]), self.assertRaises(PageHistoryError):
                parse_history_list(html)
        with self.assertRaises(PageHistoryError):
            parse_history_pages([history([revision(5, 500)], next_page=2)])


if __name__ == "__main__":
    unittest.main()
