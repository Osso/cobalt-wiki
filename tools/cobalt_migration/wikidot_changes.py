"""Site-wide revision list from Wikidot's SiteChanges module.

``changes/SiteChangesListModule`` renders 100 revisions per request, newest
first, without page sources: page, flags, time, revision number, author and
comment. Walks pages until one is empty, or until a revision already known
(``--since`` epoch) is reached, for incremental updates.

    python -m tools.cobalt_migration.wikidot_changes https://cobalt-company.wikidot.com out.json [--since EPOCH]
"""

import html
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

PER_PAGE = 100
TOKEN = "cobaltreplica"
ITEM = re.compile(r'<div class="changes-list-item">(.*?)(?=<div class="changes-list-item">|\Z)', re.S)
TITLE = re.compile(r'<td class="title">\s*<a href="/([^"]+)">(.*?)</a>', re.S)
FLAG = re.compile(r'<span class="spantip" title="[^"]*">(\w)</span>')
DATE = re.compile(r'class="odate time_(\d+)')
REVISION = re.compile(r'<td class="revision-no">\s*\((?:rev\. (\d+)|new)\)')
USER = re.compile(
    r'<td class="mod-by">\s*<span class="printuser[^"]*">.*?'
    r'user:info/([^"]+)"[^>]*userInfo\((\d+)\)[^>]*>(?:<img[^>]*>)?([^<]*)</a>',
    re.S,
)
COMMENTS = re.compile(r'<div class="comments">(.*?)</div>', re.S)


def text(fragment):
    return " ".join(html.unescape(re.sub(r"<[^>]+>", " ", fragment)).split())


def parse_changes(body):
    """Revisions listed in one SiteChangesListModule response, in order."""
    changes = []
    for item in ITEM.findall(body):
        slug, title = TITLE.search(item).groups()
        user = USER.search(item)
        comments = COMMENTS.search(item)
        changes.append({
            "slug": slug,
            # The cell reads "category: Title" outside the default category.
            "title": text(title).removeprefix(f"{slug.split(':')[0]}: " if ":" in slug else ""),
            "flags": "".join(FLAG.findall(item)),
            "changed_at": int(DATE.search(item).group(1)),
            # "(new)" is revision 0.
            "revision": int(REVISION.search(item).group(1) or 0),
            "user_slug": user.group(1) if user else None,
            "user_id": int(user.group(2)) if user else None,
            "user_name": html.unescape(user.group(3)).strip() if user else None,
            "comments": text(comments.group(1)) if comments else "",
        })
    return changes


def fetch_page(origin, page, attempts=4):
    data = urllib.parse.urlencode({
        "moduleName": "changes/SiteChangesListModule",
        "perpage": PER_PAGE,
        "page": page,
        "options": json.dumps({"all": True}),
        "wikidot_token7": TOKEN,
    }).encode()
    request = urllib.request.Request(
        f"{origin}/ajax-module-connector.php",
        data=data,
        headers={"Cookie": f"wikidot_token7={TOKEN}"},
    )
    for attempt in range(attempts):
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                reply = json.load(response)
            if reply.get("status") != "ok":
                raise RuntimeError(f"SiteChanges page {page}: {reply.get('status')}")
            return reply["body"]
        except (urllib.error.URLError, TimeoutError) as error:
            if attempt + 1 == attempts:
                raise
            print(f"retrying page {page}: {error}", file=sys.stderr)
            time.sleep(2 ** attempt)


def fetch_changes(origin, since=None, interval=1.0):
    changes, page = [], 1
    while True:
        rows = parse_changes(fetch_page(origin, page))
        fresh = [row for row in rows if since is None or row["changed_at"] > since]
        changes.extend(fresh)
        print(f"page {page}: {len(changes)} revisions", file=sys.stderr)
        if not rows or len(fresh) < len(rows):
            return changes
        page += 1
        time.sleep(interval)


if __name__ == "__main__":
    origin, output = sys.argv[1].rstrip("/"), sys.argv[2]
    since = int(sys.argv[4]) if len(sys.argv) > 4 and sys.argv[3] == "--since" else None
    with open(output, "w") as file:
        json.dump(fetch_changes(origin, since), file, indent=1)
