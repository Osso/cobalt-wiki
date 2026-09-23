"""Acquire source history from the existing protected latest-source import plan."""

import argparse
import hashlib
import json
from pathlib import Path

from .history_export import export_history
from .listing_export import _prepare_path, _save_checkpoint


def _inventory(plan):
    pages = plan.get("pages")
    if not isinstance(pages, list) or not pages:
        raise ValueError("import plan requires pages")
    identities, unresolved = [], []
    for position, page in enumerate(pages):
        if page.get("metadata_status") != "accepted":
            unresolved.append(
                {"position": position, "status": page.get("metadata_status")}
            )
            continue
        identity = page.get("metadata", {}).get("page_id")
        if type(identity) is not int or identity <= 0:
            raise ValueError("accepted source page requires positive identity")
        identities.append(identity)
    if len(set(identities)) != len(identities):
        raise ValueError("duplicate source page identity")
    digest = hashlib.sha256(
        json.dumps([identities, unresolved], sort_keys=True).encode()
    ).hexdigest()
    return identities, unresolved, digest


def _progress(state):
    return {
        **state,
        "listed_revisions": sum(page["listed"] for page in state["pages"].values()),
        "acquired_bodies": sum(page["bodies"] for page in state["pages"].values()),
    }


def acquire_site_history(
    plan, source_origin, archive_directory, fetch, *, exporter=export_history
):
    """Validate cached page archives on resume; do not refetch completed records."""
    identities, unresolved, digest = _inventory(plan)
    directory = Path(archive_directory)
    path = directory / "site-progress.json"
    _prepare_path(path)
    identity = {"schema": 1, "source_origin": source_origin, "inventory_sha256": digest}
    if path.exists():
        previous = json.loads(path.read_text())
        if any(previous.get(key) != value for key, value in identity.items()):
            raise ValueError("site history archive identity mismatch")
    state = {
        **identity,
        "unresolved_metadata": unresolved,
        "pages": {},
        "failed_page_id": None,
    }
    for page_id in identities:
        try:
            page = exporter(source_origin, page_id, directory / str(page_id), fetch)
        except Exception:
            state["failed_page_id"] = page_id
            _save_checkpoint(path, _progress(state))
            raise
        state["pages"][str(page_id)] = {
            "listed": len(page["revisions"]),
            "bodies": sum(body["status"] == 200 for body in page["bodies"].values()),
            "unavailable_bodies": sum(
                body["status"] != 200 for body in page["bodies"].values()
            ),
            "unobserved_ranges": page["unobserved_ranges"],
        }
        _save_checkpoint(path, _progress(state))
    if not identities:
        _save_checkpoint(path, _progress(state))
    return _progress(state)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", required=True, type=Path)
    parser.add_argument("--source-origin", required=True)
    parser.add_argument("--archive-directory", required=True, type=Path)
    parser.add_argument("--target-id", required=True)
    parser.add_argument("--node-binary", required=True)
    args = parser.parse_args(argv)
    from .history_transport import make_history_fetch

    fetch = make_history_fetch(
        args.source_origin, args.target_id, node_binary=args.node_binary
    )
    result = acquire_site_history(
        json.loads(args.plan.read_text()),
        args.source_origin,
        args.archive_directory,
        fetch,
    )
    print(
        json.dumps(
            {
                key: result[key]
                for key in ("listed_revisions", "acquired_bodies", "failed_page_id")
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
