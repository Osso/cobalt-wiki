"""Create-only World Anvil Boromir transport; no update or delete operations."""

import json
import math
import random
import time
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener
from uuid import UUID


class WorldAnvilError(Exception):
    """A sanitized failure with an optional provider response for diagnostics."""

    def __init__(self, message, *, response_body=None):
        super().__init__(message)
        self.response_body = response_body


def _uuid(value):
    if not isinstance(value, str):
        return False
    try:
        return str(UUID(value)) == value.lower()
    except ValueError:
        return False


def _article(value):
    return (
        isinstance(value, dict)
        and _uuid(value.get("id"))
        and isinstance(value.get("title"), str)
        and bool(value["title"].strip())
        and value.get("success") is not False
    )


def _retry_delay(attempt, retry_after):
    delay = min(8, 2**attempt + random.uniform(0, 0.25))
    if retry_after is not None:
        try:
            delay = max(delay, float(retry_after))
        except ValueError:
            try:
                date = parsedate_to_datetime(retry_after)
                delay = max(delay, (date - datetime.now(timezone.utc)).total_seconds())
            except (TypeError, ValueError, OverflowError):
                pass
    if not math.isfinite(delay) or delay > 30:
        raise WorldAnvilError("read retry delay exceeds limit") from None
    return delay


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, newurl):
        return None


class WorldAnvilClient:
    """Four allowed operations against one World Anvil API origin."""

    def __init__(
        self,
        application_key,
        auth_token,
        user_agent,
        *,
        base_url="https://www.worldanvil.com/api/external/boromir",
        timeout=15,
    ):
        parts = urlsplit(base_url)
        if (
            (
                parts.scheme != "https"
                and not (
                    parts.scheme == "http"
                    and parts.hostname in {"127.0.0.1", "localhost"}
                )
            )
            or parts.username
            or parts.password
            or parts.query
            or parts.fragment
            or not parts.netloc
        ):
            raise ValueError("invalid World Anvil API URL")
        if (
            not isinstance(timeout, (int, float))
            or not math.isfinite(timeout)
            or timeout <= 0
        ):
            raise ValueError("invalid request timeout")
        if not all(
            isinstance(value, str) and value and "\r" not in value and "\n" not in value
            for value in (application_key, auth_token, user_agent)
        ):
            raise ValueError("invalid API credentials or user agent")
        self._url = base_url.rstrip("/")
        self._headers = {
            "x-application-key": application_key,
            "x-auth-token": auth_token,
            "Content-Type": "application/json",
            "User-Agent": user_agent,
        }
        self._timeout = timeout
        self._opener = build_opener(_NoRedirect)

    def _request(self, method, path, *, query=None, body=None, read=True):
        url = self._url + path
        if query is not None:
            url += "?" + urlencode(query)
        data = (
            json.dumps(body).encode()
            if body is not None
            else (b"{}" if method == "POST" else None)
        )
        request = Request(url, data=data, headers=self._headers, method=method)
        for attempt in range(4 if read else 1):
            try:
                with self._opener.open(request, timeout=self._timeout) as response:
                    if response.status not in (200, 201):
                        raise WorldAnvilError(
                            f"World Anvil {method} {path} returned HTTP {response.status}"
                        )
                    result = json.load(response)
                    if not isinstance(result, (dict, list)) or (
                        isinstance(result, dict) and result.get("success") is False
                    ):
                        raise WorldAnvilError(
                            f"invalid World Anvil {method} {path} response"
                        )
                    return result
            except HTTPError as error:
                with error:
                    status = error.code
                    retry_after = error.headers.get("Retry-After")
                    response_body = error.read().decode("utf-8", errors="replace")
                for name in ("x-application-key", "x-auth-token"):
                    response_body = response_body.replace(
                        self._headers[name], "[REDACTED]"
                    )
                if read and status in (408, 429, 500, 502, 503, 504) and attempt < 3:
                    time.sleep(_retry_delay(attempt, retry_after))
                    continue
                raise WorldAnvilError(
                    f"World Anvil {method} {path} returned HTTP {status}",
                    response_body=response_body,
                ) from None
            except (TimeoutError, URLError, ConnectionError, OSError):
                if read and attempt < 3:
                    time.sleep(_retry_delay(attempt, None))
                    continue
                raise WorldAnvilError(
                    f"World Anvil {method} {path} connection failed"
                ) from None
            except (ValueError, UnicodeError):
                raise WorldAnvilError(
                    f"invalid World Anvil {method} {path} JSON"
                ) from None

    def list_articles(self, world_id):
        if not _uuid(world_id):
            raise ValueError("invalid world ID")
        articles = []
        seen = set()
        while True:
            page = self._request(
                "POST",
                "/world/articles",
                query={"id": world_id},
                body={"offset": len(articles), "limit": 50},
            )
            if not isinstance(page, dict) or page.get("success") is not True:
                raise WorldAnvilError("invalid World Anvil article page envelope")
            page = page.get("entities")
            if not isinstance(page, list) or len(page) > 50:
                raise WorldAnvilError("invalid World Anvil article page")
            for article in page:
                if not _article(article) or article["id"] in seen:
                    raise WorldAnvilError("invalid or duplicate World Anvil article")
                seen.add(article["id"])
                articles.append(article)
            if len(page) < 50:
                return articles

    def get_article(self, article_id):
        if not _uuid(article_id):
            raise ValueError("invalid article ID")
        article = self._request(
            "GET", "/article", query={"id": article_id, "granularity": 2}
        )
        if not _article(article) or article["id"] != article_id:
            raise WorldAnvilError("invalid World Anvil article")
        return article

    def get_world(self, world_id):
        if not _uuid(world_id):
            raise ValueError("invalid world ID")
        world = self._request("GET", "/world", query={"id": world_id, "granularity": 2})
        if not _article(world) or world["id"] != world_id:
            raise WorldAnvilError("invalid World Anvil world")
        return world

    def create_article(self, world_id, article):
        if not _uuid(world_id):
            raise ValueError("invalid world ID")
        if (
            not isinstance(article, dict)
            or "id" in article
            or "world" in article
            or not isinstance(article.get("title"), str)
            or not article["title"].strip()
            or not isinstance(article.get("templateType"), str)
            or not article["templateType"]
        ):
            raise ValueError("invalid new article")
        result = self._request(
            "PUT", "/article", body={**article, "world": {"id": world_id}}, read=False
        )
        if not _article(result):
            raise WorldAnvilError("invalid World Anvil created article")
        return result
