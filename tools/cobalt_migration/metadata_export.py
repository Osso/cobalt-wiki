"""Acquire allowlisted metadata using canonical names from a listing checkpoint."""

import hashlib
import json
from pathlib import Path
import random
import time
from urllib.parse import quote, urlsplit

from .browser_transport import SourcePageRedirect
from .listing_export import _fetch_page, _prepare_path, _save_checkpoint
from .page_metadata import SourcePageUnavailable, parse_page_metadata


class MetadataExportError(ValueError):
    """Metadata acquisition cannot safely resume."""


def _validate_input(origin, names):
    parts = urlsplit(origin)
    if (
        parts.scheme not in {"http", "https"}
        or not parts.hostname
        or parts.username is not None
        or parts.password is not None
        or parts.path
        or parts.query
        or parts.fragment
    ):
        raise MetadataExportError("source_origin must be an HTTP(S) origin")
    if not isinstance(names, list) or any(
        not isinstance(name, str)
        or not name
        or name in {".", ".."}
        or any(c.isspace() or ord(c) < 32 or c in "/\\" for c in name)
        for name in names
    ):
        raise MetadataExportError("expected a list of canonical page names")
    keys = [name.replace(":", "_") for name in names]
    if len(set(keys)) != len(keys):
        raise MetadataExportError("duplicate names or colliding archive keys")
    return hashlib.sha256(
        json.dumps(names, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    ).hexdigest()


def _load_checkpoint(path, origin, digest, names):
    initial = dict(
        schema=1,
        source_origin=origin,
        names_sha256=digest,
        completed_position=0,
        records=[],
    )
    if not path.exists():
        return initial
    try:
        state = json.loads(path.read_text())
    except (ValueError, UnicodeError):
        raise MetadataExportError("invalid metadata checkpoint JSON") from None
    if not isinstance(state, dict) or set(state) != set(initial):
        raise MetadataExportError("invalid metadata checkpoint fields")
    if (
        type(state["schema"]) is not int
        or state["schema"] != 1
        or state["source_origin"] != origin
        or state["names_sha256"] != digest
    ):
        raise MetadataExportError("checkpoint schema, origin or names digest mismatch")
    position, records = state["completed_position"], state["records"]
    if (
        type(position) is not int
        or not 0 <= position <= len(names)
        or not isinstance(records, list)
        or len(records) != position
    ):
        raise MetadataExportError("invalid checkpoint completed position")
    for name, record in zip(names, records):
        _validate_record(name, record)
    return state


def _validate_record(name, record):
    common = {"fullname", "archive_key", "status"}
    if not isinstance(record, dict):
        raise MetadataExportError("invalid checkpoint record")
    status = record.get("status")
    fields = common
    if status == "accepted":
        fields = common | {"page_id", "title", "tags", "revision_number", "updated_at"}
    if (
        status not in {"accepted", "denied", "not_found", "redirect"}
        or set(record) != fields
        or record["fullname"] != name
        or record["archive_key"] != name.replace(":", "_")
    ):
        raise MetadataExportError("checkpoint record identity or outcome mismatch")


def _parse_record(html, name):
    identity = {"fullname": name, "archive_key": name.replace(":", "_")}
    try:
        metadata = parse_page_metadata(html, expected_fullname=name)
    except SourcePageUnavailable as error:
        if error.reason not in {"denied", "not_found"}:
            raise
        return {**identity, "status": error.reason}
    return {**metadata, "archive_key": identity["archive_key"], "status": "accepted"}


def export_metadata(
    source_origin,
    fullnames,
    checkpoint_path,
    fetch,
    *,
    sleep=time.sleep,
    jitter=random.random,
    now=time.time,
):
    """FetchResponse callback receives quoted paths; pass listing['fullnames'].

    Fixed one-second GET spacing and bounded retries match listing acquisition.
    A completed position includes classified unavailable pages, not just accepted
    metadata. No HTML or session state is written to the checkpoint.
    """
    names = list(fullnames) if isinstance(fullnames, list) else fullnames
    digest = _validate_input(source_origin, names)
    path = Path(checkpoint_path)
    _prepare_path(path)
    state = _load_checkpoint(path, source_origin, digest, names)
    previous_get = False
    for name in names[state["completed_position"] :]:
        try:
            html = _fetch_page(
                fetch,
                "/" + quote(name, safe=":"),
                sleep,
                1.0,
                previous_get,
                jitter,
                now,
            )
        except SourcePageRedirect:
            record = {
                "fullname": name,
                "archive_key": name.replace(":", "_"),
                "status": "redirect",
            }
        else:
            record = _parse_record(html, name)
        previous_get = True
        state = {
            **state,
            "completed_position": state["completed_position"] + 1,
            "records": [*state["records"], record],
        }
        _save_checkpoint(path, state)
    return state
