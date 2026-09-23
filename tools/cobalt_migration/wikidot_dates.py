"""Page creation and last-edit times from Wikidot's ListPages module.

Wikidot's site backup has no page dates. Its AJAX module connector renders
ListPages with any body, so one request lists up to 250 pages as
``fullname|created_at|updated_at``; dates come from the ``time_<epoch>``
class of Wikidot's ``odate`` spans.

    python -m tools.cobalt_migration.wikidot_dates https://cobalt-company.wikidot.com out.json
"""

import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

PER_PAGE = 250
TOKEN = "cobaltreplica"
ROW = re.compile(
    r'<span class="date-row">([^|<]+)\|<span class="odate time_(\d+)[^"]*">[^<]*</span>'
    r'\|<span class="odate time_(\d+)[^"]*">[^<]*</span></span>'
)
PAGE_COUNT = re.compile(r'<span class="pager-no">page \d+ of (\d+)</span>')


def parse_dates(body):
    """Rows and total page count from one ListPages response body."""
    rows = {name: {"created_at": int(created), "updated_at": int(updated)}
            for name, created, updated in ROW.findall(body)}
    pages = PAGE_COUNT.search(body)
    return rows, int(pages.group(1)) if pages else 1


def request_body(page):
    return urllib.parse.urlencode({
        "moduleName": "list/ListPagesModule",
        "category": "*",
        "pagetype": "*",
        "perPage": PER_PAGE,
        "order": "created_at",
        "separate": "no",
        "p": page,
        "module_body": '[[span class="date-row"]]%%fullname%%|%%created_at%%|%%updated_at%%[[/span]]',
        "wikidot_token7": TOKEN,
    }).encode()


def fetch_page(origin, page, attempts=4):
    request = urllib.request.Request(
        f"{origin}/ajax-module-connector.php",
        data=request_body(page),
        headers={"Cookie": f"wikidot_token7={TOKEN}"},
    )
    for attempt in range(attempts):
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                reply = json.load(response)
            if reply.get("status") != "ok":
                raise RuntimeError(f"ListPages page {page}: {reply.get('status')}")
            return reply["body"]
        except (urllib.error.URLError, TimeoutError) as error:
            if attempt + 1 == attempts:
                raise
            time.sleep(2 ** attempt)
            print(f"retrying page {page}: {error}", file=sys.stderr)


def fetch_dates(origin, interval=1.0):
    dates, page, pages = {}, 1, 1
    while page <= pages:
        rows, pages = parse_dates(fetch_page(origin, page))
        dates.update(rows)
        print(f"page {page}/{pages}: {len(dates)} pages", file=sys.stderr)
        page += 1
        time.sleep(interval)
    return dates


if __name__ == "__main__":
    origin, output = sys.argv[1].rstrip("/"), sys.argv[2]
    with open(output, "w") as file:
        json.dump(fetch_dates(origin), file, indent=1, sort_keys=True)
