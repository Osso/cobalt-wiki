"""Resumable private archive of one authorized page's Wikidot history."""

import hashlib
import json
import os
import random
import stat
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urlsplit

from .history_source import HistorySourceError, decode_history_source
from .listing_export import (
    ListingExportError,
    _prepare_path,
    _retry_delay,
    _save_checkpoint,
)
from .page_history import PageHistoryError, parse_history_list, parse_history_pages


class HistoryExportError(ValueError):
    """History acquisition cannot safely continue."""


@dataclass(frozen=True)
class HistoryResponse:
    status: int
    raw: str
    html: str
    retry_after: str | None = None


def _digest(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _save_text(path, value):
    _prepare_path(path)
    fd, name = tempfile.mkstemp(prefix=".history-", dir=path.parent)
    temporary = Path(name)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as output:
            output.write(value)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def _record_response(directory, stem, response):
    for suffix, value in (("raw", response.raw), ("html", response.html)):
        _save_text(directory / f"{stem}.{suffix}", value)
    return {"raw_sha256": _digest(response.raw), "html_sha256": _digest(response.html)}


def _read_response(directory, stem, hashes):
    if not isinstance(hashes, dict) or not {"raw_sha256", "html_sha256"} <= set(hashes):
        raise HistoryExportError("invalid archived response hashes")
    values = []
    for suffix in ("raw", "html"):
        path = directory / f"{stem}.{suffix}"
        try:
            _prepare_path(path)
            value = path.read_text(encoding="utf-8")
        except (OSError, UnicodeError, ListingExportError):
            raise HistoryExportError(
                "missing or unprotected archived response"
            ) from None
        if _digest(value) != hashes[f"{suffix}_sha256"]:
            raise HistoryExportError("archived response hash mismatch")
        values.append(value)
    return values[1]


def _load(directory, origin, page_id):
    path = directory / "checkpoint.json"
    _prepare_path(path)
    if not path.exists():
        return {
            "schema": 1,
            "source_origin": origin,
            "source_page_id": page_id,
            "list_pages": [],
            "next_page": 1,
            "revisions": None,
            "unobserved_ranges": None,
            "bodies": {},
        }
    try:
        state = json.loads(path.read_text(encoding="utf-8"))
    except (ValueError, UnicodeError):
        raise HistoryExportError("invalid history checkpoint JSON") from None
    if (
        not isinstance(state, dict)
        or set(state)
        != {
            "schema",
            "source_origin",
            "source_page_id",
            "list_pages",
            "next_page",
            "revisions",
            "unobserved_ranges",
            "bodies",
        }
        or type(state["schema"]) is not int
        or state["schema"] != 1
        or state["source_origin"] != origin
        or state["source_page_id"] != page_id
    ):
        raise HistoryExportError("history checkpoint identity or schema mismatch")
    pages = state["list_pages"]
    if (
        not isinstance(pages, list)
        or not isinstance(state["bodies"], dict)
        or state["next_page"]
        not in ((len(pages) + 1, None) if state["revisions"] is None else (None,))
    ):
        raise HistoryExportError("invalid history checkpoint progress")
    try:
        if any(type(entry.get("acquired_at")) not in (int, float) for entry in pages):
            raise HistoryExportError("invalid list acquisition time")
        html_pages = [
            _read_response(directory, f"list-{i}", hashes)
            for i, hashes in enumerate(pages, 1)
        ]
        if state["revisions"] is None:
            if state["unobserved_ranges"] is not None or state["bodies"]:
                raise HistoryExportError("bodies require complete list inventory")
            if html_pages:
                parsed = parse_history_list(html_pages[-1])
                if (
                    parsed["page"] != len(pages)
                    or parsed["next_page"] != state["next_page"]
                ):
                    raise HistoryExportError("checkpoint pagination mismatch")
        else:
            inventory = parse_history_pages(html_pages)
            if state["revisions"] != inventory["revisions"] or state[
                "unobserved_ranges"
            ] != [list(x) for x in inventory["unobserved_ranges"]]:
                raise HistoryExportError("history checkpoint inventory mismatch")
            for revision_id, entry in state["bodies"].items():
                if revision_id not in {
                    str(r["revision_id"]) for r in inventory["revisions"]
                }:
                    raise HistoryExportError("unknown body in history checkpoint")
                html = _read_response(
                    directory, f"revision-{revision_id}", entry["hashes"]
                )
                if "module_status" in entry:
                    # An explicit refusal: no body was returned.
                    if "wikitext" in entry or html:
                        raise HistoryExportError("invalid archived source refusal")
                elif entry["status"] == 200:
                    decoded = decode_history_source(html)
                    if any(entry.get(key) != value for key, value in decoded.items()):
                        raise HistoryExportError(
                            "archived body differs from checkpoint"
                        )
                elif entry["status"] not in (403, 404) or "wikitext" in entry:
                    raise HistoryExportError("invalid archived body outcome")
                if not isinstance(entry["acquired_at"], (int, float)):
                    raise HistoryExportError("invalid acquisition time")
        return state
    except (
        KeyError,
        TypeError,
        ValueError,
        PageHistoryError,
        HistorySourceError,
        ListingExportError,
    ) as error:
        if isinstance(error, HistoryExportError):
            raise
        raise HistoryExportError("invalid history checkpoint or archive") from error


def _fetch(fetch, request, previous, sleep, jitter, now):
    delay = 1 if previous else 0
    for attempt in range(4):
        if delay:
            sleep(delay)
        try:
            response = fetch(request)
        except (TimeoutError, ConnectionError):
            response = None
        if response is not None:
            if (
                type(response.status) is not int
                or not 100 <= response.status <= 599
                or not isinstance(response.raw, str)
                or not isinstance(response.html, str)
            ):
                raise HistoryExportError("invalid history response")
            if response.status in (200, 403, 404):
                return response
            if response.status != 429 and not 500 <= response.status <= 599:
                raise HistoryExportError(
                    f"history request failed: HTTP {response.status}"
                )
        if attempt == 3:
            raise HistoryExportError(
                "history request exhausted four attempts"
            ) from None
        try:
            retry_after = _retry_delay(response.retry_after, now) if response else 0
        except ListingExportError as error:
            raise HistoryExportError(str(error)) from error
        delay = max(1, 2**attempt + jitter(), retry_after)
    raise AssertionError("unreachable")


def _finish_inventory(directory, state):
    pages = [
        _read_response(directory, f"list-{i}", hashes)
        for i, hashes in enumerate(state["list_pages"], 1)
    ]
    try:
        inventory = parse_history_pages(pages)
    except PageHistoryError as error:
        raise HistoryExportError("invalid or inconsistent history list") from error
    state["revisions"] = inventory["revisions"]
    state["unobserved_ranges"] = [list(x) for x in inventory["unobserved_ranges"]]
    state["next_page"] = None
    _save_checkpoint(directory / "checkpoint.json", state)


def _module_status(raw):
    """The `status` of a module connector JSON response, if it is one."""
    try:
        reply = json.loads(raw)
    except ValueError:
        return None
    return reply.get("status") if isinstance(reply, dict) else None


def export_history(
    source_origin,
    source_page_id,
    archive_directory,
    fetch,
    *,
    sleep=time.sleep,
    jitter=random.random,
    now=time.time,
):
    """Archive one page. fetch(request dict) returns HistoryResponse-like fields.

    Completed list pages and bodies never refetch; raw and HTML files are
    persisted before checkpoint advancement. Checkpoint is returned on success.
    """
    parts = urlsplit(source_origin) if isinstance(source_origin, str) else None
    if (
        parts is None
        or parts.scheme not in ("https", "http")
        or not parts.hostname
        or parts.username
        or parts.password
        or parts.path
        or parts.query
        or parts.fragment
        or type(source_page_id) is not int
        or source_page_id <= 0
    ):
        raise HistoryExportError("invalid source origin or page ID")
    directory = Path(archive_directory)
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    if (
        directory.is_symlink()
        or directory.stat().st_uid != os.getuid()
        or stat.S_IMODE(directory.stat().st_mode) & 0o077
    ):
        raise HistoryExportError("history archive must be owner-only")
    try:
        state = _load(directory, source_origin, source_page_id)
    except ListingExportError as error:
        raise HistoryExportError(str(error)) from error
    previous = False
    while state["revisions"] is None:
        page = state["next_page"]
        if page is None:
            _finish_inventory(directory, state)
            break
        response = _fetch(
            fetch,
            {
                "moduleName": "history/PageRevisionListModule",
                "page": page,
                "perpage": 20,
                "page_id": source_page_id,
                "options": {"all": True},
            },
            previous,
            sleep,
            jitter,
            now,
        )
        previous = True
        acquired_at = now()
        hashes = _record_response(directory, f"list-{page}", response)
        if response.status != 200:
            raise HistoryExportError(
                f"history list unavailable: HTTP {response.status}"
            )
        try:
            parsed = parse_history_list(response.html)
            if parsed["page"] != page or parsed["previous_page"] != (
                page - 1 if page > 1 else None
            ):
                raise PageHistoryError("history list pagination mismatch")
        except PageHistoryError as error:
            raise HistoryExportError("invalid or denied history list") from error
        state["list_pages"].append({**hashes, "acquired_at": acquired_at})
        state["next_page"] = parsed["next_page"]
        _save_checkpoint(directory / "checkpoint.json", state)
        if parsed["next_page"] is None:
            _finish_inventory(directory, state)
    for revision in state["revisions"]:
        identity = str(revision["revision_id"])
        if identity in state["bodies"]:
            continue
        response = _fetch(
            fetch,
            {
                "moduleName": "history/PageSourceModule",
                "revision_id": revision["revision_id"],
            },
            previous,
            sleep,
            jitter,
            now,
        )
        previous = True
        acquired_at = now()
        hashes = _record_response(directory, f"revision-{identity}", response)
        module_status = _module_status(response.raw)
        if response.status == 200 and module_status not in (None, "ok"):
            # Wikidot refused this source explicitly (e.g. a category whose
            # sources are private): an explicit gap, not a transport failure.
            decoded = {"module_status": module_status}
        elif response.status == 200:
            try:
                decoded = decode_history_source(response.html)
            except HistorySourceError as error:
                raise HistoryExportError(
                    "invalid or denied historical source"
                ) from error
        else:
            decoded = {}
        state["bodies"][identity] = {
            "status": response.status,
            "hashes": hashes,
            "acquired_at": acquired_at,
            **decoded,
        }
        _save_checkpoint(directory / "checkpoint.json", state)
    return state
