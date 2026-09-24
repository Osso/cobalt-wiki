import unittest

from tools.cobalt_migration.wikidot_files import (
    FileListError,
    check_download,
    file_events,
    format_size,
    parse_file_list,
    plan_files,
)

# cobalt-company.wikidot.com PageFilesModule for
# writing:2026-09-14-carrot-cake-and-a-luckydo (captured 2026-09-24).
TWO_FILES = """<h1>Files</h1>

	<p>
		Total files: 2
		<br/>
		Total size: 256.84 kB
	</p>

	<table class="table table-striped table-hover page-files">
		<thead>
		  <tr>
			  <th>File name</th>
			  <th>File type</th>
			  <th>Size</th>
			  <th></th>
		  </tr>
		</thead>

    <tbody>
					<tr id="file-row-11905682">
				<td>
					<a href="/local--files/writing:2026-09-14-carrot-cake-and-a-luckydo/Baird%20Cosmology.png">Baird Cosmology.png</a>
				</td>
				<td>
					<span title="PNG image data, 773 x 823, 8-bit/color RGBA, non-interlaced">PNG image data</span>
				</td>
				<td>
					202.42 kB
				</td>
				<td>
          <a href="javascript:;" onclick="WIKIDOT.modules.PageFilesModule.listeners.fileMoreInfo(event, 11905682)" class="btn btn-info btn-sm btn-small"><i class='icon-info'></i> Info</a>
          <a href="javascript:;" onclick="toggleFileOptions(11905682)" class="btn btn-primary btn-sm btn-small"><i class="icon-plus"></i> Options</a>
				</td>
			</tr>
					<tr id="file-row-11905681">
				<td>
					<a href="/local--files/writing:2026-09-14-carrot-cake-and-a-luckydo/BairdCursive.png">BairdCursive.png</a>
				</td>
				<td>
					<span title="PNG image data, 773 x 316, 8-bit/color RGBA, non-interlaced">PNG image data</span>
				</td>
				<td>
					54.42 kB
				</td>
				<td>
          <a href="javascript:;" onclick="toggleFileOptions(11905681)" class="btn btn-primary btn-sm btn-small"><i class="icon-plus"></i> Options</a>
				</td>
			</tr>
				</tbody>
	</table>

<div style="text-align: center">

</div>
<div id="files-page-id" style="display: none">1469327422</div>"""

# Page "toc": no attachments.
NO_FILES = """<h1>Files</h1>

	<p>
		No files attached to this page
	</p>
<div id="files-page-id" style="display: none">1453232511</div>"""

# Page "icons", first of five listing pages (one row kept).
FIRST_OF_FIVE = """<p>
		Total files: 468
		<br/>
		Total size: 11.07 MB
	</p>
	<tbody>
					<tr id="file-row-11787309">
				<td>
					<a href="/local--files/icons/icon_abigael.jpg">icon_abigael.jpg</a>
				</td>
				<td>
					<span title="JPEG image data, progressive, precision 8, 144x144, components 3">JPEG image data</span>
				</td>
				<td>
					29.56 kB
				</td>
			</tr>
	</tbody>
<div class="pager"><span class="pager-no">page 1 of 5</span><span class="current">1</span></div>"""

# What wdfiles.com serves, with HTTP 200, for a file that does not exist.
FILE_DOES_NOT_EXIST = b"""<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN"
     "http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd">
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="en" lang="en">
    <head>
        <title>The file does not exist</title>
    </head>
    <body>
        <p>The file does not exist.</p>
    </body>
</html>
"""


def row(slug, comments, flags="F"):
    return {"slug": slug, "flags": flags, "comments": comments}


class FormatSizeTest(unittest.TestCase):
    def test_matches_wikidot_display(self):
        # Replica byte counts of files Wikidot lists with these sizes.
        self.assertEqual(format_size(30274), "29.56 kB")
        self.assertEqual(format_size(207275), "202.42 kB")
        self.assertEqual(format_size(55728), "54.42 kB")

    def test_units_trailing_zeros_and_half_up(self):
        self.assertEqual(format_size(0), "0 Bytes")
        self.assertEqual(format_size(1023), "1023 Bytes")
        self.assertEqual(format_size(1024), "1 kB")
        self.assertEqual(format_size(17101), "16.7 kB")
        self.assertEqual(format_size(1152), "1.13 kB")  # 1.125 rounds up, as PHP's round()
        self.assertEqual(format_size(3 * 1024 ** 2), "3 MB")


class ParseFileListTest(unittest.TestCase):
    def test_reads_names_links_and_sizes(self):
        files, total, pages = parse_file_list(TWO_FILES)
        self.assertEqual(files, {
            "Baird Cosmology.png": {
                "href": "/local--files/writing:2026-09-14-carrot-cake-and-a-luckydo/Baird%20Cosmology.png",
                "size": "202.42 kB",
            },
            "BairdCursive.png": {
                "href": "/local--files/writing:2026-09-14-carrot-cake-and-a-luckydo/BairdCursive.png",
                "size": "54.42 kB",
            },
        })
        self.assertEqual((total, pages), (2, 1))

    def test_page_without_files(self):
        self.assertEqual(parse_file_list(NO_FILES), ({}, 0, 1))

    def test_paged_listing_reports_total_and_pages(self):
        files, total, pages = parse_file_list(FIRST_OF_FIVE)
        self.assertEqual(list(files), ["icon_abigael.jpg"])
        self.assertEqual((total, pages), (468, 5))

    def test_unknown_shape_is_an_error(self):
        with self.assertRaises(FileListError):
            parse_file_list("<h1>Files</h1><p>Something else</p>")


class FileEventsTest(unittest.TestCase):
    def test_reads_every_wikidot_file_comment(self):
        rows = [
            row("icons", 'Uploaded file "icon_isabeau.jpg".'),
            row("icons", 'File "icon_brynnal.jpg" renamed to "icon_baird.jpg".'),
            row("icons", 'File "old icon.png" deleted'),
            row("character:atley", 'File "Dane2.png" moved from page "deleted:character:atley".'),
            row("character:atley", 'File "Dane.png" moved away to page "character:dane".'),
            row("character:atley", "Something new Wikidot writes"),
            row("icons", 'Uploaded file "ignored.jpg".', flags="S"),
        ]
        self.assertEqual(file_events(rows), {
            "icons": {
                "touched": {"icon_isabeau.jpg", "icon_baird.jpg"},
                "gone": {"icon_brynnal.jpg", "old icon.png"},
                "unrecognized": [],
            },
            "character:atley": {
                "touched": {"Dane2.png"},
                "gone": {"Dane.png"},
                "unrecognized": ["Something new Wikidot writes"],
            },
        })


class PlanFilesTest(unittest.TestCase):
    WIKIDOT = {
        "new.png": {"size": "1 kB"},
        "touched.png": {"size": "2 kB"},
        "same.png": {"size": "3 kB"},
        "resized.png": {"size": "5 kB"},
    }
    REPLICA = {
        "touched.png": {"size": 2048},
        "same.png": {"size": 3072},
        "resized.png": {"size": 4096},
        "deleted.png": {"size": 10},
        "unexplained.png": {"size": 10},
    }

    def test_decides_each_file(self):
        self.assertEqual(
            plan_files(self.WIKIDOT, self.REPLICA, touched={"touched.png"}, gone={"deleted.png"}),
            {
                "new.png": "create",
                "touched.png": "verify",
                "resized.png": "verify",
                "deleted.png": "delete",
                "unexplained.png": "keep",
            },
        )

    def test_gone_file_still_listed_is_not_deleted(self):
        # Deleted then uploaded again in the same window: Wikidot lists it.
        plan = plan_files({"a.png": {"size": "1 kB"}}, {"a.png": {"size": 1024}},
                          touched={"a.png"}, gone={"a.png"})
        self.assertEqual(plan, {"a.png": "verify"})


class CheckDownloadTest(unittest.TestCase):
    def test_refuses_wikidot_missing_file_page(self):
        with self.assertRaisesRegex(FileListError, "does not exist"):
            check_download(FILE_DOES_NOT_EXIST, "29.56 kB")

    def test_refuses_size_other_than_listed(self):
        with self.assertRaisesRegex(FileListError, "30000 bytes"):
            check_download(b"x" * 30000, "29.56 kB")

    def test_accepts_listed_size(self):
        check_download(b"x" * 30274, "29.56 kB")


if __name__ == "__main__":
    unittest.main()
