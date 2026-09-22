"""Sequential listing acquisition with a protected, resumable checkpoint."""

from dataclasses import dataclass
from email.utils import parsedate_to_datetime
import json
import math
import os
from pathlib import Path
import random
import stat
import tempfile
import time
from urllib.parse import urlsplit

from .page_listing import parse_page_listing


class ListingExportError(ValueError):
    """Acquisition cannot safely continue."""


@dataclass(frozen=True)
class FetchResponse:
    status: int
    html: str
    retry_after: str | None = None


def _prepare_path(path):
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    parent = path.parent.stat()
    if parent.st_uid != os.getuid() or stat.S_IMODE(parent.st_mode) & 0o077:
        raise ListingExportError("checkpoint parent must be owner-only")
    if path.is_symlink():
        raise ListingExportError("checkpoint must not be a symlink")
    if path.exists():
        info = path.stat()
        if (
            not stat.S_ISREG(info.st_mode)
            or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o600
        ):
            raise ListingExportError("checkpoint must be an owner-owned 0600 file")


def _load_checkpoint(path, origin):
    if not path.exists():
        return dict(
            schema=1,
            source_origin=origin,
            completed_page=0,
            highest_page=1,
            fullnames=[],
        )
    try:
        state = json.loads(path.read_text())
    except (ValueError, UnicodeError):
        raise ListingExportError("invalid checkpoint JSON") from None
    keys = {"schema", "source_origin", "completed_page", "highest_page", "fullnames"}
    if not isinstance(state, dict) or set(state) != keys:
        raise ListingExportError("invalid checkpoint fields")
    if (
        type(state["schema"]) is not int
        or state["schema"] != 1
        or state["source_origin"] != origin
    ):
        raise ListingExportError("checkpoint schema or source origin mismatch")
    completed, highest, names = (
        state["completed_page"],
        state["highest_page"],
        state["fullnames"],
    )
    if (
        type(completed) is not int
        or type(highest) is not int
        or not 0 <= completed <= highest
        or highest < 1
    ):
        raise ListingExportError("invalid checkpoint pagination")
    if (
        not isinstance(names, list)
        or any(not isinstance(n, str) or not n for n in names)
        or len(set(names)) != len(names)
    ):
        raise ListingExportError("invalid checkpoint fullnames")
    return state


def _save_checkpoint(path, state):
    fd, name = tempfile.mkstemp(prefix=".listing-", dir=path.parent)
    temporary = Path(name)
    try:
        with os.fdopen(fd, "w") as output:
            json.dump(state, output, ensure_ascii=False)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def _retry_delay(value, now):
    if value is None:
        return 0
    try:
        if value.strip().isdigit():
            return float(value.strip())
        date = parsedate_to_datetime(value)
        if date.tzinfo is None:
            raise ValueError
        return max(0, date.timestamp() - now())
    except (ValueError, TypeError, OverflowError):
        raise ListingExportError("invalid Retry-After header") from None


def _fetch_page(fetch, path, sleep, interval, previous_get, jitter, now):
    delay = interval if previous_get else 0
    for attempt in range(4):
        if delay:
            sleep(delay)
        response = None
        try:
            response = fetch(path)
        except (ConnectionError, TimeoutError):
            pass
        if response is not None:
            if response.status == 200:
                return response.html
            if response.status != 429 and not 500 <= response.status <= 599:
                raise ListingExportError(f"listing GET failed: HTTP {response.status}")
        if attempt == 3:
            raise ListingExportError("listing GET exhausted four attempts") from None
        retry_after = _retry_delay(response.retry_after, now) if response else 0
        delay = max(interval, 2**attempt + jitter(), retry_after)
    raise AssertionError("unreachable retry state")


def export_listing(
    source_origin,
    checkpoint_path,
    fetch,
    *,
    sleep=time.sleep,
    interval_seconds=1.0,
    jitter=random.random,
    now=time.time,
):
    """FetchResponse callback receives only /pagelist/p/N paths.

    ConnectionError and TimeoutError denote retryable transport failures.
    Return the checkpoint dict; never persist response HTML.
    """
    parts = urlsplit(source_origin)
    if (
        parts.scheme not in {"https", "http"}
        or not parts.hostname
        or parts.username
        or parts.password
        or parts.path
        or parts.query
        or parts.fragment
    ):
        raise ListingExportError("source_origin must be an HTTP(S) origin")
    if not math.isfinite(interval_seconds) or interval_seconds < 0:
        raise ListingExportError("interval_seconds must be finite and nonnegative")
    path = Path(checkpoint_path)
    _prepare_path(path)
    state = _load_checkpoint(path, source_origin)
    names = dict.fromkeys(state["fullnames"])
    previous_get = False
    while state["completed_page"] < state["highest_page"]:
        page = state["completed_page"] + 1
        html = _fetch_page(
            fetch,
            f"/pagelist/p/{page}",
            sleep,
            interval_seconds,
            previous_get,
            jitter,
            now,
        )
        previous_get = True
        parsed = parse_page_listing(html, source_origin)
        names.update(dict.fromkeys(parsed["fullnames"]))
        state = dict(
            schema=1,
            source_origin=source_origin,
            completed_page=page,
            highest_page=max(state["highest_page"], parsed["highest_page"]),
            fullnames=list(names),
        )
        _save_checkpoint(path, state)
    return state
