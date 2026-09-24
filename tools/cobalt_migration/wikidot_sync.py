"""Bring the replica up to date with pages changed on Wikidot.

From Wikidot's revision list newer than ``SINCE`` (epoch), in this order:

1. Renames. Wikidot writes ``R`` revisions with the fixed comment
   ``You successfully renamed the page: "old" to "new".``; the replica page
   at ``old`` is moved to ``new`` (page_move) when only ``old`` exists.
   Moving to ``deleted:…`` is how Wikidot's non-permanent delete works and is
   mirrored the same way.
2. Pages. For each changed page, read the current source
   (``viewsource/ViewSourceModule``), title, tags and dates (a one-page
   ListPages), then create or edit the replica page as the import principal.
   Pages whose source Wikidot denies anonymously are reported, not guessed.
3. Files, for pages with ``F`` revisions: Wikidot's complete file list
   (``files/PageFilesModule``) against the replica's files. Missing files are
   downloaded from wdfiles.com and created; files touched in the window (or
   whose displayed size differs) are downloaded and replaced when the bytes
   differ; replica files absent from Wikidot are deleted only when a revision
   says the file was deleted, renamed or moved away.
4. Deletions, when ``REPLICA_PAGES`` is given: a JSON list of live replica
   slugs that have rows in the replica's copy of Wikidot's revision list
   (``wikidot_site_change``), so pages that only ever existed on the replica
   are never candidates. A permanent Wikidot delete removes the page and all
   its revisions, so SiteChanges never shows it. Listed pages missing from
   Wikidot's full ListPages listing are deleted (page_delete) only when the
   import principal created them and Wikidot answers 404 "does not exist".

Writes ``sync-report.json`` (everything done or skipped, with reasons) and
``sync-apply.sql`` (page dates, new revision-list rows, revision-list rows
moved to renamed pages and removed for deleted pages), to run with psql on the
replica database. Every Wikidot request waits 1s after the previous one.

    python -m tools.cobalt_migration.wikidot_sync https://cobalt-company.wikidot.com \\
        http://127.0.0.1:27471/jsonrpc 6000000 PASSWORD_FILE SINCE_EPOCH OUT_DIR [REPLICA_PAGES]

The Deepwell URL must be loopback (an SSH forward to the replica host).
"""

import hashlib
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
from .wikidot_dates import fetch_dates
from .wikidot_files import FileListError, check_download, file_events, parse_file_list, plan_files

TOKEN = "cobaltreplica"
PAGE_ID = re.compile(r"WIKIREQUEST\.info\.pageId\s*=\s*(\d+);")
META = re.compile(
    r'<span class="sync-meta">(.*?)\|(.*?)\|(.*?)\|<span class="odate time_(\d+)[^"]*">[^<]*</span>'
    r'\|<span class="odate time_(\d+)[^"]*">[^<]*</span></span>',
    re.S,
)
RENAMED = re.compile(r'You successfully renamed the page: "([^"]+)" to "([^"]+)"\.')
# More deletion candidates than this means the listing, not the site, changed.
MAX_DELETIONS = 25
NOTHING_TO_MOVE = "nothing to move: neither slug on the replica (the page sync creates the new one)"
INTERVAL = 1.0
ATTEMPTS = 4
_last_request = 0.0


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


def throttle():
    """Keep INTERVAL seconds between Wikidot requests."""
    global _last_request
    wait = _last_request + INTERVAL - time.monotonic()
    if wait > 0:
        time.sleep(wait)
    _last_request = time.monotonic()


def paced(fetch, *args, **kwargs):
    """Run a multi-request fetcher (which paces its own requests) so its first
    and last requests also keep INTERVAL from this module's requests."""
    global _last_request
    throttle()
    try:
        return fetch(*args, interval=INTERVAL, **kwargs)
    finally:
        _last_request = time.monotonic()


def wikidot_request(request):
    """(HTTP status, body bytes). Retries connection failures, timeouts, 429
    and 5xx a bounded number of times; other HTTP errors are answers."""
    for attempt in range(ATTEMPTS):
        throttle()
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return response.status, response.read()
        except urllib.error.HTTPError as error:
            body = error.read()
            if error.code != 429 and error.code < 500:
                return error.code, body
            failure = error
        except (urllib.error.URLError, TimeoutError) as error:
            failure = error
        if attempt + 1 == ATTEMPTS:
            raise failure
        print(f"retrying {getattr(request, 'full_url', request)}: {failure}", file=sys.stderr)
        time.sleep(2 ** attempt)
    raise AssertionError("unreachable")


def module(origin, **params):
    params["wikidot_token7"] = TOKEN
    request = urllib.request.Request(
        f"{origin}/ajax-module-connector.php",
        data=urllib.parse.urlencode(params).encode(),
        headers={"Cookie": f"wikidot_token7={TOKEN}"},
    )
    status, body = wikidot_request(request)
    if status != 200:
        raise RuntimeError(f"{params['moduleName']}: http {status}")
    return json.loads(body)


def wikidot_page(origin, slug):
    """The page's current state on Wikidot, or a reason it is unavailable."""
    status, body = wikidot_request(f"{origin}/{slug}")
    if status != 200:
        return {"status": f"http {status}"}
    page_id = PAGE_ID.search(body.decode())
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
    return {
        "status": "ok",
        "page_id": int(page_id.group(1)),
        "source": decode_view_source(source["body"]),
        **meta,
    }


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


def parse_renames(changes):
    """Renames in the revision list, oldest first. Each row is listed under
    the page's current slug; the comment holds the names at the time."""
    renames = []
    for row in sorted(changes, key=lambda row: (row["changed_at"], row["revision"])):
        if "R" not in row["flags"]:
            continue
        match = RENAMED.fullmatch(row["comments"])
        renames.append({
            "from": match.group(1) if match else None,
            "to": match.group(2) if match else None,
            "revision": row["revision"],
            "changed_at": row["changed_at"],
            "comments": row["comments"],
        })
    return renames


def plan_rename(rename, old_exists, new_exists):
    """What to do for one rename, given which slugs exist on the replica."""
    if rename["from"] is None:
        return "skip: rename comment not recognized"
    if old_exists and new_exists:
        return "skip: both slugs exist on the replica"
    if old_exists:
        return "move"
    if new_exists:
        return "already moved"
    return NOTHING_TO_MOVE


def sync_renames(rpc, site_id, user_id, renames):
    """Apply renames in order; records each one's outcome on it."""
    for rename in renames:
        old = new = None
        if rename["from"] is not None:
            old = rpc.rpc("page_get", {"site_id": site_id, "page": rename["from"]})
            new = rpc.rpc("page_get", {"site_id": site_id, "page": rename["to"]})
        action = plan_rename(rename, old is not None, new is not None)
        if action == "move":
            rpc.rpc("page_move", {
                "site_id": site_id, "page": old["page_id"], "last_revision_id": old["revision_id"],
                "new_slug": rename["to"], "user_id": user_id, "ip_address": "127.0.0.1",
                "revision_comments": f"Wikidot sync (rev. {rename['revision']}, renamed)",
            })
            action = "moved"
        rename["outcome"] = action


def file_url(origin, href):
    """wdfiles.com URL for a listed ``/local--files/page/name`` link, encoded
    as Wikidot's own redirect does (``:`` as %3A); wdfiles answers 500 for
    some names when the colon is raw."""
    parts = urllib.parse.urlsplit(origin)
    path = urllib.parse.quote(urllib.parse.unquote(href), safe="/")
    return f"{parts.scheme}://{parts.hostname.removesuffix('.wikidot.com')}.wdfiles.com{path}"


def list_wikidot_files(origin, page_id):
    """The page's complete file list on Wikidot, or FileListError."""
    files, page, pages, total = {}, 1, 1, 0
    while page <= pages:
        reply = module(origin, moduleName="files/PageFilesModule", page_id=page_id, page=page)
        if reply.get("status") != "ok":
            raise FileListError(f"PageFilesModule: {reply.get('status')}")
        rows, total, pages = parse_file_list(reply["body"])
        files.update(rows)
        page += 1
    if len(files) != total:
        raise FileListError(f"listed {len(files)} of {total} files")
    return files


def download(url, listed_size):
    """The file's bytes; FileListError (reported per file) when retries run out
    or the bytes are not the listed file."""
    try:
        status, data = wikidot_request(url)
    except (urllib.error.URLError, TimeoutError) as error:
        raise FileListError(f"download failed: {error}") from None
    if status != 200:
        raise FileListError(f"download: http {status}")
    check_download(data, listed_size)
    return data


def upload_blob(rpc, user_id, data):
    upload = rpc.rpc("blob_upload", {"user_id": user_id, "blob_size": len(data)})
    rpc.put(upload["presign_url"], data)
    return upload["pending_blob_id"]


def sync_files(rpc, site_id, user_id, origin, replica_page_id, wikidot_page_id, events):
    """Make the replica page's files match Wikidot's; {name: outcome}."""
    wikidot = list_wikidot_files(origin, wikidot_page_id)
    replica = {
        file["name"]: file
        for file in rpc.rpc("page_get_files", {"site_id": site_id, "page_id": replica_page_id, "deleted": False})
    }
    comment = "Wikidot sync (file)"
    outcomes = {}
    for name, action in sorted(plan_files(wikidot, replica, events["touched"], events["gone"]).items()):
        try:
            if action in {"create", "verify"}:
                data = download(file_url(origin, wikidot[name]["href"]), wikidot[name]["size"])
                current = replica.get(name)
                if current and hashlib.sha512(data).hexdigest() == current["s3_hash"]:
                    outcomes[name] = "unchanged"
                    continue
                blob = upload_blob(rpc, user_id, data)
                common = {"site_id": site_id, "page_id": replica_page_id, "user_id": user_id,
                          "revision_comments": comment, "bypass_filter": True, "ip_address": "127.0.0.1"}
                if current is None:
                    rpc.rpc("file_create", {**common, "name": name, "uploaded_blob_id": blob})
                    outcomes[name] = "created"
                else:
                    rpc.rpc("file_edit", {**common, "file_id": current["file_id"],
                                          "last_revision_id": current["revision_id"],
                                          "uploaded_blob_id": blob})
                    outcomes[name] = "updated"
            elif action == "delete":
                current = replica[name]
                rpc.rpc("file_delete", {
                    "site_id": site_id, "page_id": replica_page_id, "file": current["file_id"],
                    "last_revision_id": current["revision_id"], "user_id": user_id,
                    "revision_comments": comment,
                })
                outcomes[name] = "deleted"
            else:
                outcomes[name] = "kept: absent on Wikidot, but no revision in the window removed it"
        except FileListError as error:
            outcomes[name] = f"skipped: {error}"
    return outcomes


def deletion_candidates(replica_slugs, wikidot_slugs, handled):
    """Replica pages absent from Wikidot's full listing, excluding slugs this
    run already handled."""
    return sorted(set(replica_slugs) - set(wikidot_slugs) - set(handled))


def missing_on_wikidot(status, body, slug):
    """Whether Wikidot's answer for /slug says the page does not exist."""
    notice = f"The page <em>{html.escape(slug)}</em> you want to access does not exist."
    return status == 404 and notice in body


def sync_deletions(rpc, site_id, user_id, origin, candidates):
    """Delete candidates Wikidot confirms gone; {slug: outcome}."""
    outcomes = {}
    for slug in candidates:
        page = rpc.rpc("page_get", {"site_id": site_id, "page": slug})
        if page is None:
            outcomes[slug] = "already gone from the replica"
            continue
        first = rpc.rpc("page_revision_get", {"site_id": site_id, "page_id": page["page_id"], "revision_number": 0})
        if first is None or first["user_id"] != user_id:
            outcomes[slug] = "kept: not created by the import principal"
            continue
        status, body = wikidot_request(f"{origin}/{urllib.parse.quote(slug, safe=':')}")
        if not missing_on_wikidot(status, body.decode(errors="replace"), slug):
            outcomes[slug] = f"kept: Wikidot answers http {status} without saying the page does not exist"
            continue
        rpc.rpc("page_delete", {
            "site_id": site_id, "page": page["page_id"], "last_revision_id": page["revision_id"],
            "user_id": user_id, "ip_address": "127.0.0.1",
            "revision_comments": "Wikidot sync (deleted on Wikidot)",
        })
        outcomes[slug] = "deleted"
    return outcomes


def sql_literal(value):
    if value is None:
        return "NULL"
    if isinstance(value, int):
        return str(value)
    return "'" + str(value).replace("'", "''") + "'"


def apply_sql(site_id, pages, changes, renames=(), deleted=()):
    """Revision-list rows follow renamed pages and leave deleted ones (Wikidot
    lists rows under the current slug and drops a deleted page's rows), then
    page dates and new revision-list rows, as one transaction."""
    lines = ["\\set ON_ERROR_STOP on", "BEGIN;"]
    for rename in renames:
        # Rows of a page are listed under its current slug, whichever way the
        # replica got there; with both slugs present the owner is unclear.
        if rename["outcome"] in {"moved", "already moved", NOTHING_TO_MOVE}:
            lines.append(
                f"UPDATE wikidot_site_change SET page_slug = {sql_literal(rename['to'])} "
                f"WHERE site_id = {site_id} AND page_slug = {sql_literal(rename['from'])};"
            )
    for slug in deleted:
        lines.append(
            f"DELETE FROM wikidot_site_change WHERE site_id = {site_id} AND page_slug = {sql_literal(slug)};"
        )
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


def run_deletions(rpc, site_id, user_id, origin, replica_pages_file, handled):
    """Deletion step and its report section."""
    with open(replica_pages_file) as file:
        replica_slugs = json.load(file)
    wikidot_slugs = paced(fetch_dates, origin)
    candidates = deletion_candidates(replica_slugs, wikidot_slugs, handled)
    if len(candidates) > MAX_DELETIONS:
        return {"skipped": f"{len(candidates)} replica pages missing from Wikidot's listing "
                           f"(limit {MAX_DELETIONS}); listing suspect, nothing deleted",
                "candidates": candidates}
    return {"wikidot_pages": len(wikidot_slugs), "pages": sync_deletions(rpc, site_id, user_id, origin, candidates)}


def main(argv):
    origin, endpoint, site_id, password_file, since, out_dir, *replica_pages = argv
    site_id, since = int(site_id), int(since)
    password = open(password_file).read().strip()
    login = LoopbackRpc(endpoint, "", site_id).rpc("login", {
        "name_or_email": "cobalt-import", "password": password,
        "ip_address": "127.0.0.1", "user_agent": "cobalt-wikidot-sync",
    })
    rpc = LoopbackRpc(endpoint, login["session_token"], site_id)
    user_id = rpc.rpc("session_get", [login["session_token"]])["user_id"]

    changes = paced(fetch_changes, origin, since=since)
    renames = parse_renames(changes)
    sync_renames(rpc, site_id, user_id, renames)

    report = {"pages": {}, "renames": renames, "files": {}, "deletions": {}}
    pages = {}
    for slug in dict.fromkeys(row["slug"] for row in changes):
        state = wikidot_page(origin, slug)
        if state["status"] != "ok":
            report["pages"][slug] = f"skipped: {state['status']}"
            continue
        newest = max(row["revision"] for row in changes if row["slug"] == slug)
        report["pages"][slug] = sync_page(rpc, site_id, user_id, slug, state, f"Wikidot sync (rev. {newest})")
        pages[slug] = state

    for slug, events in file_events(changes).items():
        entry = report["files"][slug] = {"unrecognized_comments": events["unrecognized"]}
        if slug not in pages:
            entry["skipped"] = "page not synced"
            continue
        replica_page = rpc.rpc("page_get", {"site_id": site_id, "page": slug})
        try:
            entry["files"] = sync_files(
                rpc, site_id, user_id, origin, replica_page["page_id"], pages[slug]["page_id"], events)
        except FileListError as error:
            entry["skipped"] = str(error)

    deleted = []
    if replica_pages:
        handled = set(report["pages"]) | {r["from"] for r in renames if r["from"]}
        report["deletions"] = run_deletions(rpc, site_id, user_id, origin, replica_pages[0], handled)
        deleted = [slug for slug, outcome in report["deletions"].get("pages", {}).items() if outcome == "deleted"]
    else:
        report["deletions"] = {"skipped": "no replica page list given"}

    # The wrapper rerenders SiteChanges pages when the revision list changed.
    report["changed"] = bool(changes or deleted)
    with open(f"{out_dir}/sync-report.json", "w") as file:
        json.dump(report, file, indent=1)
    with open(f"{out_dir}/sync-apply.sql", "w") as file:
        file.write(apply_sql(site_id, pages, changes, renames, deleted))
    for rename in renames:
        print(f"rename {rename['from']} -> {rename['to']}: {rename['outcome']}")
    for slug, outcome in report["pages"].items():
        print(f"{slug}: {outcome}")
    for slug, entry in report["files"].items():
        for name, outcome in entry.get("files", {}).items():
            print(f"{slug} file {name}: {outcome}")
        if "skipped" in entry:
            print(f"{slug} files skipped: {entry['skipped']}")
        for comments in entry["unrecognized_comments"]:
            print(f"{slug} unrecognized file change: {comments}")
    for slug, outcome in report["deletions"].get("pages", {}).items():
        print(f"delete {slug}: {outcome}")
    if "skipped" in report["deletions"]:
        print(f"deletions: {report['deletions']['skipped']}")


if __name__ == "__main__":
    main(sys.argv[1:])
