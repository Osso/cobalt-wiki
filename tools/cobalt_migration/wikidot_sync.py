"""Bring replica pages up to date with pages changed on Wikidot.

For each page in Wikidot's revision list newer than ``--since`` (epoch), read
the current source (``viewsource/ViewSourceModule``), title, tags and dates
(a one-page ListPages), then create or edit the replica page through Deepwell
as the import principal. Pages whose source Wikidot denies anonymously are
reported, not guessed. Writes the SQL that stores the page dates and the new
revision-list rows, to run with psql on the replica database.

    python -m tools.cobalt_migration.wikidot_sync https://cobalt-company.wikidot.com \\
        http://127.0.0.1:27471/jsonrpc 6000000 PASSWORD_FILE SINCE_EPOCH OUT_DIR

The Deepwell URL must be loopback (an SSH forward to the replica host).
"""

import html
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

from .poc_import import LoopbackRpc
from .wikidot_changes import fetch_changes

TOKEN = "cobaltreplica"
PAGE_ID = re.compile(r"WIKIREQUEST\.info\.pageId\s*=\s*(\d+);")
META = re.compile(
    r'<span class="sync-meta">(.*?)\|(.*?)\|(.*?)\|<span class="odate time_(\d+)[^"]*">[^<]*</span>'
    r'\|<span class="odate time_(\d+)[^"]*">[^<]*</span></span>',
    re.S,
)


def decode_view_source(body):
    """Page source from a ViewSourceModule response. Wikidot displays it as
    HTML: `<br />` before each newline, include targets linked, runs of spaces
    as `&nbsp;`, and the saved source is trimmed."""
    marker = '<div class="page-source">'
    inner = body[body.index(marker) + len(marker):body.rindex("</div>")]
    inner = re.sub(r"<br\s*/?>", "", inner)
    inner = re.sub(r"<a [^>]*>(.*?)</a>", r"\1", inner, flags=re.S)
    if re.search(r"<\w", inner):
        raise ValueError("unexpected markup in page source")
    return html.unescape(inner.replace("&nbsp;", " ")).strip()


def parse_meta(body):
    """Title, tags (visible and hidden, sorted) and dates from the ListPages body."""
    match = META.search(body)
    if match is None:
        return None
    title, tags, hidden, created, updated = match.groups()
    return {
        "title": html.unescape(title),
        "tags": sorted(set(html.unescape(tags).split() + html.unescape(hidden).split())),
        "created_at": int(created),
        "updated_at": int(updated),
    }


def module(origin, **params):
    params["wikidot_token7"] = TOKEN
    request = urllib.request.Request(
        f"{origin}/ajax-module-connector.php",
        data=urllib.parse.urlencode(params).encode(),
        headers={"Cookie": f"wikidot_token7={TOKEN}"},
    )
    for attempt in range(4):
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return json.load(response)
        except (urllib.error.URLError, TimeoutError):
            if attempt == 3:
                raise
            time.sleep(2 ** attempt)


def wikidot_page(origin, slug):
    """The page's current state on Wikidot, or a reason it is unavailable."""
    try:
        with urllib.request.urlopen(f"{origin}/{slug}", timeout=60) as response:
            page = response.read().decode()
    except urllib.error.HTTPError as error:
        return {"status": f"http {error.code}"}
    page_id = PAGE_ID.search(page)
    if page_id is None:
        return {"status": "no page id"}
    source = module(origin, moduleName="viewsource/ViewSourceModule", page_id=page_id.group(1))
    if source.get("status") != "ok":
        return {"status": source.get("status")}
    listing = module(
        origin,
        moduleName="list/ListPagesModule",
        fullname=slug,
        category="*",
        pagetype="*",
        separate="no",
        module_body='[[span class="sync-meta"]]%%title%%|%%tags%%|%%_tags%%|%%created_at%%|%%updated_at%%[[/span]]',
    )
    meta = parse_meta(listing.get("body", ""))
    if meta is None:
        return {"status": "no listing"}
    return {"status": "ok", "source": decode_view_source(source["body"]), **meta}


def sync_page(rpc, site_id, user_id, slug, state, comment):
    """Create or edit the replica page to match; returns what was done."""
    page = rpc.rpc("page_get", {"site_id": site_id, "page": slug, "details": {"wikitext": True}})
    if page is None:
        rpc.rpc("page_import", {
            "site_id": site_id, "user_id": user_id, "slug": slug,
            "title": state["title"], "wikitext": state["source"], "tags": state["tags"],
            "alt_title": None, "layout": None, "revision_comments": comment,
            "bypass_filter": True, "ip_address": "127.0.0.1",
        })
        return "created"
    changes = {}
    if page["wikitext"] != state["source"]:
        changes["wikitext"] = state["source"]
    if page["title"] != state["title"]:
        changes["title"] = state["title"]
    if sorted(page["tags"]) != state["tags"]:
        changes["tags"] = state["tags"]
    if not changes:
        return "unchanged"
    rpc.rpc("page_edit", {
        "site_id": site_id, "user_id": user_id, "page": page["page_id"],
        "last_revision_id": page["revision_id"], "revision_comments": comment,
        "ip_address": "127.0.0.1", **changes,
    })
    return "edited " + ",".join(sorted(changes))


def sql_literal(value):
    if value is None:
        return "NULL"
    if isinstance(value, int):
        return str(value)
    return "'" + str(value).replace("'", "''") + "'"


def apply_sql(site_id, pages, changes):
    """Page dates and new revision-list rows, as one transaction."""
    lines = ["\\set ON_ERROR_STOP on", "BEGIN;"]
    for slug, state in pages.items():
        lines.append(
            f"UPDATE page SET created_at = to_timestamp({state['created_at']}), "
            f"updated_at = to_timestamp({state['updated_at']}) "
            f"WHERE site_id = {site_id} AND slug = {sql_literal(slug)} AND deleted_at IS NULL;"
        )
    for row in changes:
        values = ", ".join(sql_literal(value) for value in [
            site_id, row["slug"], row["title"], row["revision"], row["flags"],
        ])
        lines.append(
            "INSERT INTO wikidot_site_change (site_id, page_slug, page_title, revision_number, "
            "flags, changed_at, user_slug, user_name, source_user_id, comments) VALUES "
            f"({values}, to_timestamp({row['changed_at']}), {sql_literal(row['user_slug'])}, "
            f"{sql_literal(row['user_name'])}, {sql_literal(row['user_id'])}, "
            f"{sql_literal(row['comments'])}) ON CONFLICT DO NOTHING;"
        )
    lines.append("COMMIT;")
    return "\n".join(lines) + "\n"


def main(argv):
    origin, endpoint, site_id, password_file, since, out_dir = argv
    site_id, since = int(site_id), int(since)
    password = open(password_file).read().strip()
    login = LoopbackRpc(endpoint, "", site_id).rpc("login", {
        "name_or_email": "cobalt-import", "password": password,
        "ip_address": "127.0.0.1", "user_agent": "cobalt-wikidot-sync",
    })
    rpc = LoopbackRpc(endpoint, login["session_token"], site_id)
    user_id = rpc.rpc("session_get", [login["session_token"]])["user_id"]

    changes = fetch_changes(origin, since=since)
    report, pages = {}, {}
    for slug in dict.fromkeys(row["slug"] for row in changes):
        state = wikidot_page(origin, slug)
        if state["status"] != "ok":
            report[slug] = f"skipped: {state['status']}"
            continue
        newest = max(row["revision"] for row in changes if row["slug"] == slug)
        report[slug] = sync_page(rpc, site_id, user_id, slug, state, f"Wikidot sync (rev. {newest})")
        pages[slug] = state
        time.sleep(1)
    with open(f"{out_dir}/sync-report.json", "w") as file:
        json.dump(report, file, indent=1)
    with open(f"{out_dir}/sync-apply.sql", "w") as file:
        file.write(apply_sql(site_id, pages, changes))
    for slug, outcome in report.items():
        print(f"{slug}: {outcome}")


if __name__ == "__main__":
    main(sys.argv[1:])
