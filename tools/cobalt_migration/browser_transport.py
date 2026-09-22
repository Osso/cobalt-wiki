"""Authenticated GET transport through the installed browser-cli executable."""

import json
import math
import subprocess
import time
from urllib.parse import urlsplit
from uuid import uuid4

from .listing_export import FetchResponse


class BrowserTransportError(RuntimeError):
    """Browser state or CLI protocol prevents safe acquisition (not retryable)."""


def _validate_origin(origin):
    if not isinstance(origin, str) or any(c.isspace() for c in origin):
        raise ValueError("source_origin must be an HTTP(S) origin")
    try:
        parts = urlsplit(origin)
        valid = (
            parts.scheme in {"http", "https"}
            and parts.hostname
            and parts.username is None
            and parts.password is None
            and not parts.path
            and not parts.query
            and not parts.fragment
            and parts.port != 0
            and "\\" not in origin
            and not any(ord(c) < 32 or ord(c) == 127 for c in origin)
        )
    except ValueError:
        valid = False
    if not valid:
        raise ValueError("source_origin must be an HTTP(S) origin") from None


def _validate_path(path):
    if (
        not isinstance(path, str)
        or not path.startswith("/")
        or path.startswith("//")
        or "\\" in path
        or "#" in path
        or any(c.isspace() or ord(c) < 32 or ord(c) == 127 for c in path)
    ):
        raise ValueError("fetch requires a single-origin absolute path")


def _decode_result(output):
    try:
        result = json.loads(output)
        if isinstance(result, str):
            result = json.loads(result)
    except (ValueError, TypeError):
        raise BrowserTransportError("invalid browser-cli JSON response") from None
    if not isinstance(result, dict) or not isinstance(result.get("state"), str):
        raise BrowserTransportError("invalid browser-cli response state") from None
    if result["state"] == "origin_changed":
        raise BrowserTransportError("browser origin changed") from None
    return result


def _response(result):
    status, html, retry_after = (
        result.get("status"),
        result.get("html"),
        result.get("retry_after"),
    )
    if (
        type(status) is not int
        or not 100 <= status <= 599
        or not isinstance(html, str)
        or not (retry_after is None or isinstance(retry_after, str))
    ):
        raise BrowserTransportError("invalid browser fetch response") from None
    return FetchResponse(status, html, retry_after)


class BrowserFetch:
    def __init__(
        self,
        source_origin,
        *,
        runner=subprocess.run,
        clock=time.monotonic,
        sleep=time.sleep,
        timeout_seconds=30.0,
        poll_interval=0.1,
    ):
        _validate_origin(source_origin)
        if (
            not math.isfinite(timeout_seconds)
            or not 0 < timeout_seconds <= 2147483
            or not math.isfinite(poll_interval)
            or poll_interval <= 0
        ):
            raise ValueError("timeout and poll interval must be finite and positive")
        self.origin = source_origin
        self.runner = runner
        self.clock = clock
        self.sleep = sleep
        self.timeout = timeout_seconds
        self.interval = poll_interval

    def _eval(self, body, timeout):
        script = (
            "(() => { if (location.origin !== "
            + json.dumps(self.origin)
            + ") return {state:'origin_changed'}; "
            + body
            + " })()"
        )
        try:
            result = self.runner(
                ["browser-cli", "eval", script],
                capture_output=True,
                text=True,
                timeout=timeout,
            )
        except subprocess.TimeoutExpired:
            raise TimeoutError("browser-cli evaluation timed out") from None
        except (OSError, subprocess.SubprocessError):
            raise BrowserTransportError("browser-cli execution failed") from None
        if result.returncode != 0:
            raise BrowserTransportError("browser-cli evaluation failed") from None
        return _decode_result(result.stdout)

    def _start(self, slot, path, remaining):
        body = (
            f"const key = {json.dumps(slot)}; const path = {json.dumps(path)}; "
            "const controller = new AbortController(); "
            "const entry = {state:'pending', controller}; window[key] = entry; "
            f"entry.timer = setTimeout(() => controller.abort(), {math.ceil(remaining * 1000)}); "
            "fetch(path, {credentials:'same-origin', redirect:'error', signal:controller.signal})"
            ".then(async response => { const html = await response.text(); "
            "Object.assign(entry, {state:'done', status:response.status, html, "
            "retry_after:response.headers.get('Retry-After')}); })"
            ".catch(() => { entry.state = controller.signal.aborted ? 'timeout' : 'error'; })"
            ".finally(() => clearTimeout(entry.timer)); "
            "return {state:'started'};"
        )
        if self._eval(body, remaining)["state"] != "started":
            raise BrowserTransportError("browser fetch did not start") from None

    def _poll(self, slot, deadline):
        body = (
            f"const entry = window[{json.dumps(slot)}]; "
            "if (!entry) return {state:'missing'}; "
            "return {state:entry.state, status:entry.status, html:entry.html, "
            "retry_after:entry.retry_after};"
        )
        while True:
            remaining = deadline - self.clock()
            if remaining <= 0:
                raise TimeoutError("browser fetch deadline exceeded") from None
            result = self._eval(body, remaining)
            if self.clock() >= deadline:
                raise TimeoutError("browser fetch deadline exceeded") from None
            state = result["state"]
            if state == "done":
                return _response(result)
            if state == "timeout":
                raise TimeoutError("browser fetch timed out") from None
            if state == "error":
                raise ConnectionError("browser fetch failed") from None
            if state != "pending":
                raise BrowserTransportError("invalid browser fetch state") from None
            self.sleep(min(self.interval, deadline - self.clock()))

    def _cleanup(self, slot):
        body = (
            f"const key = {json.dumps(slot)}; const entry = window[key]; "
            "if (entry) { clearTimeout(entry.timer); entry.controller.abort(); delete window[key]; } "
            "return {state:'cleaned'};"
        )
        if self._eval(body, min(self.timeout, 5.0))["state"] != "cleaned":
            raise BrowserTransportError("browser slot cleanup failed") from None

    def __call__(self, path):
        _validate_path(path)
        slot = "__cobalt_fetch_" + uuid4().hex
        deadline = self.clock() + self.timeout
        failure = None
        try:
            self._start(slot, path, self.timeout)
            return self._poll(slot, deadline)
        except BaseException as error:
            failure = error
            raise
        finally:
            try:
                self._cleanup(slot)
            except Exception:
                if failure is not None:
                    failure.add_note("browser slot cleanup failed")
                else:
                    raise BrowserTransportError("browser slot cleanup failed") from None


def make_browser_fetch(source_origin, **options):
    """Return a FetchResponse callback; runner accepts subprocess.run keywords.

    TimeoutError/ConnectionError are retryable by export_listing. CLI/protocol
    and origin failures are permanent. Cleanup has a separate <=5 second budget.
    """
    return BrowserFetch(source_origin, **options)
