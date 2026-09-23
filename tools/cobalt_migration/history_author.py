"""Extract source-backed Wikidot author identity from a profile modal."""

import re
from urllib.parse import unquote, urlsplit

from .page_listing import _ListingHTML, _Node, _walk


class HistoryAuthorError(ValueError):
    """Profile markup does not establish a unique author identity and date."""


def _unique(items, field):
    if len(items) != 1:
        raise HistoryAuthorError(f"expected one {field}, found {len(items)}")
    return items[0]


def _children(node, tag):
    return [
        child
        for child in node.children
        if isinstance(child, _Node) and child.tag == tag
    ]


def _has_class(node, name):
    return name in (node.attrs.get("class") or "").split()


def _profile_slug(modal):
    links = [
        node.attrs.get("href")
        for node in _walk(modal)
        if node.tag == "a"
        and (node.attrs.get("href") or "").startswith(
            "https://www.wikidot.com/user:info/"
        )
    ]
    href = _unique(links, "profile link")
    parts = urlsplit(href)
    if (
        parts.scheme != "https"
        or parts.netloc != "www.wikidot.com"
        or parts.query
        or parts.fragment
        or not parts.path.startswith("/user:info/")
    ):
        raise HistoryAuthorError("invalid profile link")
    try:
        slug = unquote(parts.path.removeprefix("/user:info/"), errors="strict")
    except UnicodeError:
        raise HistoryAuthorError("profile slug is not valid UTF-8") from None
    if not slug or any(c.isspace() or ord(c) < 32 or c in "/\\?#" for c in slug):
        raise HistoryAuthorError("invalid profile slug")
    return slug


def _profile_fields(modal):
    fields = {}
    for table in (node for node in _walk(modal) if node.tag == "table"):
        for row in (node for node in _walk(table) if node.tag == "tr"):
            cells = _children(row, "td")
            if len(cells) < 2:
                continue
            label = cells[0].text().strip().rstrip(":")
            if label not in {"Wikidot.com User since", "Account type", "Karma level"}:
                continue
            if label in fields or len(cells) != 2:
                raise HistoryAuthorError(f"ambiguous {label}")
            fields[label] = cells[1]
    return fields


def _account_created_at(field):
    dates = [node for node in _walk(field) if _has_class(node, "odate")]
    date = _unique(dates, "Wikidot.com account date")
    epochs = [
        value.removeprefix("time_")
        for value in (date.attrs.get("class") or "").split()
        if value.startswith("time_")
    ]
    epoch = _unique(epochs, "Wikidot.com account epoch")
    if not re.fullmatch(r"[0-9]+", epoch) or int(epoch) <= 0:
        raise HistoryAuthorError("invalid Wikidot.com account epoch")
    return int(epoch)


def parse_history_author(html, *, user_id, fetched_at):
    """Return only sourced identity, creation time, and raw account type/karma."""
    if (
        not isinstance(html, str)
        or type(user_id) is not int
        or user_id <= 0
        or type(fetched_at) is not int
        or fetched_at <= 0
    ):
        raise HistoryAuthorError("invalid profile request identity or acquisition time")
    document = _ListingHTML()
    document.feed(html)
    modal = _unique(
        [node for node in _walk(document.root) if _has_class(node, "modal-body")],
        "profile modal",
    )
    name = _unique(
        [node.text().strip() for node in _walk(modal) if node.tag == "h1"],
        "display name",
    )
    if not name:
        raise HistoryAuthorError("missing display name")
    fields = _profile_fields(modal)
    if set(fields) != {"Wikidot.com User since", "Account type", "Karma level"}:
        raise HistoryAuthorError("missing required account fields")
    account_type = fields["Account type"].text().strip()
    karma = fields["Karma level"].text().strip()
    if not account_type or not karma:
        raise HistoryAuthorError("empty account type or karma")
    return {
        "source_author_id": user_id,
        "display_name": name,
        "slug": _profile_slug(modal),
        "created_at": _account_created_at(fields["Wikidot.com User since"]),
        "account_type": account_type,
        "karma": karma,
        "fetched_at": fetched_at,
    }
