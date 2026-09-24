"""Select absent import entries without modifying the plan or target inventory."""

from collections import Counter, defaultdict

from . import poc_import as poc


def _valid_field(field, value):
    if field.endswith("_id"):
        return type(value) is int and value > 0
    if field == "deleted":
        return type(value) is bool
    return isinstance(value, str) and bool(value)


def _records(inventory, key, fields):
    records = inventory.get(key)
    if not isinstance(records, list):
        raise TypeError(f"inventory {key} must be a complete array")
    for record in records:
        if not isinstance(record, dict):
            raise TypeError(f"inventory {key} contains a malformed record")
        for field in fields:
            value = record.get(field)
            if not _valid_field(field, value):
                raise ValueError(f"inventory {key} has invalid {field}")
    return records


def _unique_ids(records, key, id_field):
    by_id = {}
    for record in records:
        identifier = record[id_field]
        if identifier in by_id and by_id[identifier] != record:
            raise ValueError(f"inventory {key} has conflicting {id_field} {identifier}")
        by_id[identifier] = record
    return by_id.values()


def _revisions(inventory, key, fields):
    records = _records(inventory, key, fields)
    identified = [record for record in records if "revision_id" in record]
    for record in identified:
        if type(record["revision_id"]) is not int or record["revision_id"] <= 0:
            raise ValueError(f"inventory {key} has invalid revision_id")
    _unique_ids(identified, key, "revision_id")
    return records


def _index_inventory(plan, inventory):
    if (
        inventory.get("schema") != 1
        or inventory.get("site_id") != plan["site_id"]
        or inventory.get("plan_sha256") != plan["plan_sha256"]
        or not isinstance(inventory.get("rpc_endpoint"), str)
        or not inventory["rpc_endpoint"]
    ):
        raise ValueError(
            "inventory schema, site, plan digest, or RPC endpoint mismatch"
        )

    pages = _unique_ids(
        _records(
            inventory, "pages", ("page_id", "slug", "deleted", "latest_revision_id")
        ),
        "pages",
        "page_id",
    )
    page_revisions = _revisions(inventory, "page_revisions", ("page_id", "slug"))
    files = _unique_ids(
        _records(inventory, "files", ("file_id", "page_id", "name", "deleted")),
        "files",
        "file_id",
    )
    file_revisions = _revisions(
        inventory, "file_revisions", ("file_id", "page_id", "name")
    )
    orphans = inventory.get("orphan_audit_page_ids")
    if not isinstance(orphans, list) or any(
        type(value) is not int or value <= 0 for value in orphans
    ):
        raise ValueError("inventory orphan audit page IDs must be a complete array")
    if orphans:
        raise ValueError("inventory contains unmapped orphan audit page IDs")

    owners = defaultdict(set)
    current = defaultdict(list)
    for record in pages:
        owners[record["slug"]].add(record["page_id"])
        current[record["slug"]].append(record)
    for record in page_revisions:
        owners[record["slug"]].add(record["page_id"])
    file_names = {
        (record["page_id"], record["name"]) for record in (*files, *file_revisions)
    }
    return owners, current, file_names


def select_missing(plan, inventory):
    """Return only safe absent pages/files from a validated original plan.

    Existing and historical identities are never compared by content or ownership.
    Entries and their ordering are retained verbatim; this function has no I/O.
    """
    owners, current, file_names = _index_inventory(plan, inventory)
    selected_pages = []
    selected_files = []
    existing_pages = {}
    skipped = Counter()
    new_names = set()

    for entry in plan["pages"]:
        name = entry["fullname"]
        if current[name]:
            active = [record for record in current[name] if not record["deleted"]]
            if len(active) == 1:
                existing_pages[name] = active[0]["page_id"]
            skipped["present_page" if active else "retired_page"] += 1
        elif owners[name]:
            skipped["retired_page"] += 1
        else:
            selected_pages.append(entry)
            new_names.add(name)

    for entry in plan["attachments"]:
        name = entry["fullname"]
        if name in new_names:
            selected_files.append(entry)
            continue
        owner_id = existing_pages.get(name)
        if len(owners[name]) > 1 and any(
            not record["deleted"] for record in current[name]
        ):
            skipped["ambiguous_owner"] += 1
        elif owner_id is None:
            skipped["unavailable_owner"] += 1
        elif (owner_id, entry["name"]) in file_names:
            skipped["present_file"] += 1
        else:
            selected_files.append(entry)

    return {
        "pages": selected_pages,
        "attachments": selected_files,
        "existing_pages": existing_pages,
        "skipped": dict(skipped),
    }


def _create_page(rpc, plan, entry, data):
    if poc._get_page(rpc, plan, entry) is not None:
        raise poc.PocImportError("page inventory changed; reconcile before restarting")
    rpc(
        "page_import",
        {
            "site_id": plan["site_id"],
            "user_id": plan["user_id"],
            "slug": entry["fullname"],
            "title": entry["title"],
            "wikitext": data.decode("utf-8"),
            "tags": entry["tags"],
            "alt_title": None,
            "layout": None,
            "revision_comments": poc._marker(plan),
            "bypass_filter": True,
            "ip_address": "127.0.0.1",
        },
    )
    page = poc._get_page(rpc, plan, entry)
    if page is None:
        raise poc.PocImportError("created page missing on readback")
    poc._check_page(page, plan, entry)
    if page["tags"] != entry["tags"]:
        raise poc.PocImportError("created page did not retain exact tags")
    return page["page_id"]


def _create_file(rpc, put, plan, entry, data, page_id):
    page = poc._get_page(rpc, plan, entry)
    if page is None or page["page_id"] != page_id:
        raise poc.PocImportError(
            "attachment owner changed; reconcile before restarting"
        )
    if poc._get_file(rpc, plan, entry, page_id) is not None:
        raise poc.PocImportError("file inventory changed; reconcile before restarting")
    poc._import_file(rpc, put, plan, entry, data, page_id)


def apply_missing(
    archive_path, plan_path, inventory, rpc, put, *, endpoint, max_pages, max_files
):
    """Create a bounded batch against a fresh complete inventory, exclusively.

    Caller exports current and historical identities before EACH invocation and
    excludes concurrent writers. A failed mutation aborts; export again before
    restarting. Existing records are never repaired or content-checked.
    """
    plan = poc._load_plan(archive_path, plan_path)
    if inventory.get("rpc_endpoint") != endpoint:
        raise ValueError("inventory RPC endpoint mismatch")
    if any(type(n) is not int or n < 0 for n in (max_pages, max_files)):
        raise ValueError("batch limits must be nonnegative integers")
    selected = select_missing(plan, inventory)
    pages = dict(selected["existing_pages"])
    entries = selected["pages"][:max_pages]
    for entry, data in poc._members(archive_path, entries):
        pages[entry["fullname"]] = _create_page(rpc, plan, entry, data)
    files = [e for e in selected["attachments"] if e["fullname"] in pages][:max_files]
    for entry, data in poc._members(archive_path, files):
        _create_file(rpc, put, plan, entry, data, pages[entry["fullname"]])
    return {"pages": len(entries), "attachments": len(files)}
