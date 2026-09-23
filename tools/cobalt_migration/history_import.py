"""Import an acquired site history archive into Deepwell's imported history.

Reads each page directory written by history_acquire.py (one per Wikidot
page ID), maps it to the replica page through the import plan's fullname, and
stores its revisions with import_wikidot_history. Revisions without a body
(sources Wikidot refused) stay explicit gaps in the report; nothing is
synthesized. Pages whose acquisition is incomplete are skipped.

    python -m tools.cobalt_migration.history_import PLAN ARCHIVE_DIR \\
        http://127.0.0.1:27471/jsonrpc 6000000 PASSWORD_FILE REPORT
"""

import datetime
import json
import sys
from pathlib import Path

from .poc_import import LoopbackRpc


def rfc3339(epoch):
    return (
        datetime.datetime.fromtimestamp(epoch, datetime.UTC)
        .isoformat()
        .replace("+00:00", "Z")
    )


def page_history(directory):
    """(revisions to import, revision numbers without a body), or None when
    the page's acquisition is incomplete."""
    directory = Path(directory)
    state = json.loads((directory / "checkpoint.json").read_text())
    revisions = state["revisions"]
    if revisions is None or len(state["bodies"]) != len(revisions):
        return None
    imported, gaps = [], []
    for revision in revisions:
        body = state["bodies"][str(revision["revision_id"])]
        if "wikitext" not in body:
            gaps.append(revision["number"])
            continue
        html = (directory / f"revision-{revision['revision_id']}.html").read_text()
        imported.append({
            "source_revision_id": revision["revision_id"],
            "source_revision_number": revision["number"],
            "source_author_id": revision["author_id"],
            "source_created_at": rfc3339(revision["created_at"]),
            "source_comments": revision["comments"] or "",
            "source_flags": [flag["marker"] for flag in revision["flags"]],
            "source_title": None,
            "source_slug": None,
            "source_tags": None,
            "wikitext": body["wikitext"],
            "raw_source_html": html,
            "acquired_at": rfc3339(body["acquired_at"]),
            "representation": body["representation"],
        })
    return imported, gaps


def main(argv):
    plan_path, archive, endpoint, site_id, password_file, report_path = argv
    site_id = int(site_id)
    fullnames = {
        page["metadata"]["page_id"]: page["fullname"]
        for page in json.loads(Path(plan_path).read_text())["pages"]
        if page.get("metadata_status") == "accepted"
    }
    password = Path(password_file).read_text().strip()
    login = LoopbackRpc(endpoint, "", site_id).rpc("login", {
        "name_or_email": "cobalt-import", "password": password,
        "ip_address": "127.0.0.1", "user_agent": "cobalt-history-import",
    })
    rpc = LoopbackRpc(endpoint, login["session_token"], site_id)
    report = {}
    for directory in sorted(Path(archive).iterdir()):
        if not (directory / "checkpoint.json").exists():
            continue
        source_page_id = int(directory.name)
        history = page_history(directory)
        slug = fullnames.get(source_page_id)
        if history is None or slug is None:
            report[str(source_page_id)] = "incomplete" if history is None else "not in plan"
            continue
        revisions, gaps = history
        page = rpc.rpc("page_get", {"site_id": site_id, "page": slug, "details": {}})
        if page is None:
            report[slug] = "missing on replica"
            continue
        result = rpc.rpc("import_wikidot_history", {
            "site_id": site_id, "page_id": page["page_id"],
            "source_page_id": source_page_id,
            "expected_revision_id": page["revision_id"],
            "revisions": revisions,
        })
        report[slug] = {"inserted": result["inserted"], "revisions": len(revisions), "gaps": gaps}
    Path(report_path).write_text(json.dumps(report, indent=1))
    inserted = sum(v["inserted"] for v in report.values() if isinstance(v, dict))
    print(f"pages {len(report)}, inserted {inserted}")


if __name__ == "__main__":
    main(sys.argv[1:])
