"""Run one Wikidot -> replica sync: renames, pages and files changed since the
newest known revision and pages deleted on Wikidot, then their dates and
revision-list rows, then rerender the pages that show SiteChanges. Invoked by
cobalt-wiki-wikidot-sync.service."""

import json
import os
import pathlib
import subprocess
import sys
import urllib.request

origin, site_id, password_file, psql, deepwell = sys.argv[1:6]
runtime = pathlib.Path(os.environ["RUNTIME_DIRECTORY"])


def query(sql):
    output = subprocess.run(
        [*psql.split(), "--tuples-only", "--no-align", "--field-separator=,", "--command", sql],
        check=True, capture_output=True, text=True,
    ).stdout
    return [line.split(",") for line in output.splitlines() if line]


def rpc(method, params):
    request = urllib.request.Request(
        deepwell,
        data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=120) as response:
        reply = json.load(response)
    if "error" in reply:
        raise RuntimeError(f"{method} failed: {reply['error'].get('message')}")


[[since]] = query(
    "SELECT COALESCE(EXTRACT(EPOCH FROM max(changed_at))::bigint, 0) "
    f"FROM wikidot_site_change WHERE site_id = {int(site_id)}"
)
# Deletion candidates: live replica pages Wikidot's revision list knows, so
# replica-only pages (fixtures, pages made on the replica) are never compared.
replica_pages = runtime / "replica-pages.json"
replica_pages.write_text(json.dumps([slug for [slug] in query(
    f"SELECT p.slug FROM page p WHERE p.site_id = {int(site_id)} AND p.deleted_at IS NULL "
    "AND EXISTS (SELECT 1 FROM wikidot_site_change c "
    "WHERE c.site_id = p.site_id AND c.page_slug = p.slug)"
)]))
subprocess.run(
    [sys.executable, "-m", "tools.cobalt_migration.wikidot_sync",
     origin, deepwell, site_id, password_file, since, str(runtime), str(replica_pages)],
    check=True,
)
subprocess.run([*psql.split(), "--set=ON_ERROR_STOP=1", "--quiet", "--file", str(runtime / "sync-apply.sql")], check=True)

if json.loads((runtime / "sync-report.json").read_text())["changed"]:
    pages = query(
        "SELECT p.site_id, p.page_category_id, p.page_id FROM page p "
        "JOIN LATERAL (SELECT wikitext_hash FROM page_revision r WHERE r.page_id = p.page_id "
        "ORDER BY revision_number DESC LIMIT 1) r ON true JOIN text t ON t.hash = r.wikitext_hash "
        f"WHERE p.site_id = {int(site_id)} AND p.deleted_at IS NULL "
        "AND t.contents ~* '\\[\\[\\s*module\\s+SiteChanges'"
    )
    for page_site, category, page in pages:
        rpc("page_rerender", {"site_id": int(page_site), "category_id": int(category),
                              "page_id": int(page), "rerender_type": "standalone"})
    print(f"rerendered {len(pages)} SiteChanges pages")
