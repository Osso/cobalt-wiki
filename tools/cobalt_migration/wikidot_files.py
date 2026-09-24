"""Page attachments: what Wikidot lists, what the file revisions say, and
what the replica must do to match.

``files/PageFilesModule`` lists a page's files 100 per request with Wikidot's
rounded size (``FileHelper::formatSize``: 1024-based, two decimals). SiteChanges
rows flagged ``F`` name the files touched by each revision in fixed English
comments. The listing is the truth for what exists; a file is deleted from the
replica only when it is absent from a complete listing *and* a revision in the
window says it was deleted, renamed away or moved away.
"""

from decimal import ROUND_HALF_UP, Decimal
import html
import re

ROW = re.compile(
    r'<tr id="file-row-\d+">\s*<td>\s*<a href="([^"]+)">([^<]*)</a>\s*</td>'
    r'\s*<td>.*?</td>\s*<td>\s*([^<]+?)\s*</td>',
    re.S,
)
TOTAL = re.compile(r"Total files: (\d+)")
PAGER = re.compile(r'<span class="pager-no">page \d+ of (\d+)</span>')
NO_FILES = "No files attached to this page"
MISSING_FILE = "<title>The file does not exist</title>"

# Comments Wikidot writes on F revisions: (pattern, touched groups, gone groups).
FILE_EVENTS = [
    (re.compile(r'Uploaded file "(.+)"\.'), (1,), ()),
    (re.compile(r'File "(.+)" deleted'), (), (1,)),
    (re.compile(r'File "(.+)" renamed to "(.+)"\.'), (2,), (1,)),
    (re.compile(r'File "(.+)" moved from page "(.+)"\.'), (1,), ()),
    (re.compile(r'File "(.+)" moved away to page "(.+)"\.'), (), (1,)),
]


class FileListError(ValueError):
    """A PageFilesModule response cannot establish the page's full file list."""


def format_size(size):
    """Wikidot's displayed size for a byte count, as PHP renders it."""
    if size == 0:
        return "0 Bytes"
    units = [" Bytes", " kB", " MB", " GB", " TB", " PB"]
    index = 0
    while size >= 1024 ** (index + 1) and index + 1 < len(units):
        index += 1
    # PHP's round() is half away from zero; size / 1024**n is exact in decimal.
    value = (Decimal(size) / 1024 ** index).quantize(Decimal("0.01"), ROUND_HALF_UP)
    return f"{value}".rstrip("0").rstrip(".") + units[index]


def parse_file_list(body):
    """Files on one listing page ({name: {"href", "size"}}), the total file
    count and the number of listing pages."""
    if NO_FILES in body:
        return {}, 0, 1
    total = TOTAL.search(body)
    if total is None:
        raise FileListError("file listing has no total")
    files = {
        html.unescape(name).strip(): {"href": html.unescape(href), "size": size}
        for href, name, size in ROW.findall(body)
    }
    pages = PAGER.search(body)
    return files, int(total.group(1)), int(pages.group(1)) if pages else 1


def file_events(rows):
    """Per page slug: file names a revision added or replaced ("touched"),
    names a revision removed from the page ("gone"), and F comments that
    match no known form ("unrecognized")."""
    events = {}
    for row in rows:
        if "F" not in row["flags"]:
            continue
        page = events.setdefault(row["slug"], {"touched": set(), "gone": set(), "unrecognized": []})
        for pattern, touched, gone in FILE_EVENTS:
            match = pattern.fullmatch(row["comments"])
            if match:
                page["touched"].update(match.group(i) for i in touched)
                page["gone"].update(match.group(i) for i in gone)
                break
        else:
            page["unrecognized"].append(row["comments"])
    return events


def plan_files(wikidot, replica, touched, gone):
    """What to do with each file name, as {name: action}.

    ``wikidot`` is the complete listing ({name: {"size"}}), ``replica`` the
    replica's current files ({name: {"size"}}). Actions: ``create`` (only on
    Wikidot), ``verify`` (download and compare bytes: touched in the window or
    the displayed size differs), ``delete`` (absent on Wikidot and a revision
    says it went away) and ``keep`` (only on the replica, no such revision).
    Files present on both, untouched and of matching size are left out.
    """
    plan = {}
    for name, listed in wikidot.items():
        if name not in replica:
            plan[name] = "create"
        elif name in touched or format_size(replica[name]["size"]) != listed["size"]:
            plan[name] = "verify"
    for name in replica.keys() - wikidot.keys():
        plan[name] = "delete" if name in gone else "keep"
    return plan


def check_download(data, listed_size):
    """Refuse Wikidot's "file does not exist" page (served with HTTP 200) and
    bytes whose size does not render as the listed size."""
    if MISSING_FILE.encode() in data[:1024]:
        raise FileListError("Wikidot says the file does not exist")
    if format_size(len(data)) != listed_size:
        raise FileListError(f"downloaded {len(data)} bytes, listing shows {listed_size}")
