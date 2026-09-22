"""Select possible duplicate rerender jobs; never modify the queue.

A repeated HSCAN candidate ID may be emitted more than once. Any later apply
or restore step must be idempotent and recheck live state before deletion.
Matching bytes alone do not establish that removing a job is semantically safe.
"""

import json


def _unique_fields(pairs):
    fields = dict(pairs)
    if len(fields) != len(pairs):
        raise ValueError("duplicate JSON field")
    return fields


def _eligible_payload(raw, site_id):
    if type(raw) is not str:
        return False
    try:
        job = json.loads(raw, object_pairs_hook=_unique_fields)
    except (TypeError, ValueError):
        return False
    if type(job) is not dict or job.keys() != {"job", "data"}:
        return False
    if job["job"] != "rerender_page" or type(job["data"]) is not dict:
        return False
    data = job["data"]
    if data.keys() != {"id", "depth", "type"} or type(data["id"]) is not dict:
        return False
    page = data["id"]
    return (
        page.keys() == {"site_id", "category_id", "page_id"}
        and type(page["site_id"]) is int
        and page["site_id"] == site_id
        and type(page["category_id"]) is int
        and page["category_id"] > 0
        and type(page["page_id"]) is int
        and page["page_id"] > 0
        and type(data["depth"]) is int
        and data["depth"] >= 0
        and data["type"] in ("full", "nav")
    )


def duplicate_candidates(records, site_id, cutoff_ms):
    """Yield keeper IDs and original candidate records for live revalidation.

    Only unreceived rerender jobs for ``site_id`` due by ``cutoff_ms``
    participate. This pure scan holds one keeper ID per distinct eligible raw
    payload, not every scanned message ID. It does not authorize deletion.
    """
    keepers = {}
    for record in records:
        message_id = record.get("id")
        if type(message_id) is not str or not message_id:
            continue
        if record.get("rc") is not None or record.get("fr") is not None:
            continue
        score = record.get("score")
        if type(score) is not str:
            continue
        try:
            due = int(score)
        except ValueError:
            continue
        if due > cutoff_ms:
            continue
        raw = record.get("payload")
        if not _eligible_payload(raw, site_id):
            continue
        keeper = keepers.setdefault(raw, message_id)
        if keeper != message_id:
            yield {"keeper_id": keeper, "record": record}
