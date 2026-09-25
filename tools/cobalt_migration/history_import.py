"""Apply only missing acquired history to guarded, existing local pages.

    python -m tools.cobalt_migration.history_import PLAN ARCHIVE_DIR \
        http://127.0.0.1:2749/jsonrpc SITE_ID PASSWORD_FILE REPORT

REPORT pins the inputs and original target revision guards. A fresh invocation
reconciles persisted records before resuming; uncertain writes are not retried.
"""

import datetime
import hashlib
import json
import sys
from pathlib import Path

from .history_acquire import _inventory
from .history_export import _load
from .listing_export import _prepare_path, _save_checkpoint
from .poc_import import LoopbackRpc, PocImportError, _digest, encode_rpc_request

MAX_REQUEST_BYTES = 10 * 1024 * 1024
IMPORT_METHOD = "import_wikidot_history"


def rfc3339(epoch):
    return (
        datetime.datetime.fromtimestamp(epoch, datetime.UTC)
        .isoformat()
        .replace("+00:00", "Z")
    )


def revision_metadata(source_page_id, revision):
    comments = revision["comments"]
    return {
        "source_page_id": source_page_id,
        "source_revision_id": revision["revision_id"],
        "source_revision_number": revision["number"],
        "source_author_id": revision["author_id"],
        "source_created_at": rfc3339(revision["created_at"]),
        "source_comments": "" if comments is None else comments,
        "source_flags": [flag["marker"] for flag in revision["flags"]],
        "source_title": None,
        "source_slug": None,
        "source_tags": None,
        "representation": "display-decoded-not-byte-exact",
    }


def load_page_history(directory, origin):
    source_id = int(directory.name)
    state = _load(directory, origin, source_id)
    revisions = state["revisions"]
    if revisions is None or len(state["bodies"]) != len(revisions):
        raise PocImportError(f"source page {source_id} acquisition is incomplete")
    metadata, records, gaps = {}, [], []
    for revision in revisions:
        revision_id = revision["revision_id"]
        summary = revision_metadata(source_id, revision)
        metadata[revision_id] = summary
        body = state["bodies"][str(revision_id)]
        if "wikitext" not in body:
            gaps.append(revision["number"])
            continue
        record = {
            key: value for key, value in summary.items() if key != "source_page_id"
        }
        record.update(
            wikitext=body["wikitext"],
            raw_source_html=(directory / f"revision-{revision_id}.html").read_text(),
            acquired_at=rfc3339(body["acquired_at"]),
        )
        records.append(record)
    return {
        "source_page_id": source_id,
        "metadata": metadata,
        "records": records,
        "gaps": gaps,
        "unobserved_ranges": state["unobserved_ranges"],
        "checkpoint_sha256": hashlib.sha256(
            (directory / "checkpoint.json").read_bytes()
        ).hexdigest(),
    }


def split_history_request(params, max_bytes=MAX_REQUEST_BYTES):
    """Pack whole records using the JSON encoder's actual byte costs, in O(bytes)."""
    empty_size = len(encode_rpc_request(IMPORT_METHOD, {**params, "revisions": []}))
    chunks, batch, size = [], [], empty_size
    for revision in params["revisions"]:
        record_size = len(json.dumps(revision).encode())
        if empty_size + record_size > max_bytes:
            raise PocImportError(
                "one historical revision exceeds the JSON-RPC request limit"
            )
        addition = record_size + (
            2 if batch else 0
        )  # Default JSON comma-space separator.
        if size + addition > max_bytes:
            chunks.append({**params, "revisions": batch})
            batch, size, addition = [], empty_size, record_size
        batch.append(revision)
        size += addition
    if batch:
        chunks.append({**params, "revisions": batch})
    return chunks


def fetch_history(rpc, page_id):
    rows, before = [], None
    while True:
        batch = rpc.rpc(
            "page_imported_history",
            {
                "site_id": rpc.site_id,
                "page_id": page_id,
                "before_revision": before,
                "limit": 100,
            },
        )
        for row in batch:
            number = row["source_revision_number"]
            if before is not None and number >= before:
                raise PocImportError("imported history pagination did not advance")
            before = number
            rows.append(row)
        if len(batch) < 100:
            return rows


def fetch_missing_history(rpc, page_id, history):
    existing = fetch_history(rpc, page_id)
    by_id = {row["source_revision_id"]: row for row in existing}
    by_number = {row["source_revision_number"]: row for row in existing}
    records = {row["source_revision_id"]: row for row in history["records"]}
    for row in existing:
        identity = row["source_revision_id"]
        if row["source_page_id"] != history["source_page_id"]:
            raise PocImportError(f"target page {page_id} has another source identity")
        expected = history["metadata"].get(identity)
        if expected is not None and row != expected:
            raise PocImportError(
                f"source revision {identity} has conflicting stored metadata"
            )
        if identity in records:
            source = rpc.rpc(
                "page_imported_revision",
                {
                    "site_id": rpc.site_id,
                    "page_id": page_id,
                    "source_revision_number": row["source_revision_number"],
                },
            )
            if source is None or source["wikitext"] != records[identity]["wikitext"]:
                raise PocImportError(
                    f"source revision {identity} has a conflicting stored body"
                )
    for identity, expected in history["metadata"].items():
        prior = by_number.get(expected["source_revision_number"])
        if prior is not None and prior["source_revision_id"] != identity:
            raise PocImportError(
                f"source revision {identity} collides with a stored number"
            )
    missing = [
        row for row in history["records"] if row["source_revision_id"] not in by_id
    ]
    unresolved = [number for number in history["gaps"] if number not in by_number]
    return missing, len(existing), unresolved


def read_json(path):
    _prepare_path(path)
    return json.loads(path.read_text())


def load_inputs(plan_path, archive, rpc):
    plan = read_json(plan_path)
    unsigned = {key: value for key, value in plan.items() if key != "plan_sha256"}
    if plan.get("schema") != 1 or plan.get("plan_sha256") != _digest(unsigned):
        raise PocImportError("source plan digest/schema mismatch")
    if plan["site_id"] != rpc.site_id:
        raise PocImportError("source plan and target site differ")
    identities, unresolved, digest = _inventory(plan)
    progress_path = archive / "site-progress.json"
    progress = read_json(progress_path)
    if progress["inventory_sha256"] != digest or progress["failed_page_id"] is not None:
        raise PocImportError("source acquisition inventory differs from the plan")
    if set(progress["pages"]) != {str(identity) for identity in identities}:
        raise PocImportError("source acquisition inventory is incomplete")
    identity = {
        "schema": 1,
        "site_id": rpc.site_id,
        "endpoint": rpc.endpoint,
        "plan_file_sha256": hashlib.sha256(plan_path.read_bytes()).hexdigest(),
        "archive_snapshot_sha256": hashlib.sha256(
            progress_path.read_bytes()
        ).hexdigest(),
    }
    entries = [page for page in plan["pages"] if page["metadata_status"] == "accepted"]
    return entries, progress, identity, unresolved


def load_prior_report(path, identity):
    _prepare_path(path)
    if not path.exists():
        return None
    report = read_json(path)
    if any(report.get(key) != value for key, value in identity.items()):
        raise PocImportError("application report belongs to different inputs or target")
    return report


def fetch_target(rpc, slug, history):
    page = rpc.rpc("page_get", {"site_id": rpc.site_id, "page": slug, "details": {}})
    if page is None or page["slug"] != slug or page.get("page_deleted_at") is not None:
        raise PocImportError(
            f"source page {history['source_page_id']} has no exact active target"
        )
    return {
        "slug": slug,
        "page_id": page["page_id"],
        "revision_id": page["revision_id"],
        "checkpoint_sha256": history["checkpoint_sha256"],
    }


def history_request(rpc, target, history, records):
    return {
        "site_id": rpc.site_id,
        "page_id": target["page_id"],
        "source_page_id": history["source_page_id"],
        "expected_revision_id": target["revision_id"],
        "revisions": records,
    }


def prepare_page(rpc, entry, archive, progress, previous):
    source_id = str(entry["metadata"]["page_id"])
    history = load_page_history(archive / source_id, progress["source_origin"])
    captured = progress["pages"][source_id]
    if (len(history["metadata"]), len(history["records"]), len(history["gaps"])) != (
        captured["listed"],
        captured["bodies"],
        captured["unavailable_bodies"],
    ):
        raise PocImportError(f"source page {source_id} differs from its inventory")
    target = fetch_target(rpc, entry["fullname"], history)
    if previous is not None and previous["targets"].get(source_id) != target:
        raise PocImportError(
            f"source page {source_id} target revision or archive changed"
        )
    missing, existing, unresolved = fetch_missing_history(
        rpc, target["page_id"], history
    )
    split_history_request(history_request(rpc, target, history, missing))
    status = {
        "available": len(history["records"]),
        "existing": existing,
        "missing": len(missing),
        "inserted": 0,
        "unavailable_bodies": len(history["gaps"]),
        "unresolved_gaps": unresolved,
        "unobserved_ranges": history["unobserved_ranges"],
    }
    return source_id, target, status


def prepare_import(plan_path, archive, rpc, report_path):
    entries, progress, identity, unresolved = load_inputs(plan_path, archive, rpc)
    previous = load_prior_report(report_path, identity)
    report = {
        **identity,
        "targets": {},
        "pages": {},
        "unresolved_metadata": unresolved,
        "available_history_applied": False,
        "failed_page_id": None,
    }
    for entry in entries:
        source_id, target, status = prepare_page(
            rpc, entry, archive, progress, previous
        )
        report["targets"][source_id] = target
        report["pages"][source_id] = status
    current_digest = hashlib.sha256(
        (archive / "site-progress.json").read_bytes()
    ).hexdigest()
    if current_digest != identity["archive_snapshot_sha256"]:
        raise PocImportError("source acquisition changed during preflight")
    return report, progress["source_origin"]


def apply_page(rpc, archive, origin, source_id, target):
    history = load_page_history(archive / source_id, origin)
    current = fetch_target(rpc, target["slug"], history)
    if current != target:
        raise PocImportError(
            f"source page {source_id} target revision or archive changed"
        )
    missing, _, _ = fetch_missing_history(rpc, target["page_id"], history)
    inserted = 0
    for request in split_history_request(
        history_request(rpc, target, history, missing)
    ):
        result = rpc.rpc(IMPORT_METHOD, request)
        if result["inserted"] != len(request["revisions"]):
            raise PocImportError(
                f"source page {source_id} history changed after preflight"
            )
        inserted += result["inserted"]
    remaining, _, _ = fetch_missing_history(rpc, target["page_id"], history)
    if remaining:
        raise PocImportError(
            f"source page {source_id} imported history readback is incomplete"
        )
    return inserted


def import_archive(plan_path, archive, rpc, report_path):
    plan_path, archive, report_path = Path(plan_path), Path(archive), Path(report_path)
    report, origin = prepare_import(plan_path, archive, rpc, report_path)
    _save_checkpoint(report_path, report)
    try:
        for index, (source_id, target) in enumerate(report["targets"].items(), 1):
            report["failed_page_id"] = source_id
            report["pages"][source_id]["inserted"] = apply_page(
                rpc, archive, origin, source_id, target
            )
            if index % 100 == 0:
                print(f"history pages processed: {index}", flush=True)
        report["failed_page_id"] = None
        report["available_history_applied"] = True
    finally:
        _save_checkpoint(report_path, report)
    return report


def main(argv):
    if len(argv) != 6:
        raise SystemExit(__doc__)
    plan_path, archive, endpoint, site_id, password_file, report_path = argv
    site_id = int(site_id)
    password_path = Path(password_file)
    _prepare_path(password_path)
    login = LoopbackRpc(endpoint, "", site_id).rpc(
        "login",
        {
            "name_or_email": "cobalt-import",
            "password": password_path.read_text().strip(),
            "ip_address": "127.0.0.1",
            "user_agent": "cobalt-history-import",
        },
    )
    rpc = LoopbackRpc(endpoint, login["session_token"], site_id)
    report = import_archive(plan_path, archive, rpc, report_path)
    inserted = sum(page["inserted"] for page in report["pages"].values())
    gaps = sum(len(page["unresolved_gaps"]) for page in report["pages"].values())
    print(
        f"pages {len(report['pages'])}, inserted {inserted}, unresolved bodies {gaps}"
    )


if __name__ == "__main__":
    main(sys.argv[1:])
