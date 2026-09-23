"""Parse Wikidot revision-list HTML without executing scripts or fetching pages."""

import re

from .page_listing import _ListingHTML, _Node, _walk


class PageHistoryError(ValueError):
    """History HTML cannot establish unambiguous revision metadata."""


def _children(node, tag):
    return [
        child
        for child in node.children
        if isinstance(child, _Node) and child.tag == tag
    ]


def _class(node, name):
    return name in (node.attrs.get("class") or "").split()


def _one(items, field):
    if len(items) != 1:
        raise PageHistoryError(f"expected one {field}, found {len(items)}")
    return items[0]


def _positive(value, field, *, allow_zero=False):
    if not re.fullmatch(r"\d+", value or ""):
        raise PageHistoryError(f"invalid {field}")
    number = int(value)
    if number == 0 and not allow_zero:
        raise PageHistoryError(f"invalid {field}")
    return number


def _handler_ids(nodes, name):
    found = []
    for node in nodes:
        onclick = node.attrs.get("onclick") or ""
        if name not in onclick:
            continue
        match = re.fullmatch(
            rf"(?:WIKIDOT\.page\.listeners\.)?{name}\((\d+)\);?\s*(?:return false;?)?",
            onclick.strip(),
        )
        if not match:
            raise PageHistoryError(f"invalid {name} handler")
        found.append(_positive(match.group(1), name))
    return found


def _optional_identity(values, field):
    if len(set(values)) > 1:
        raise PageHistoryError(f"conflicting {field}")
    return values[0] if values else None


def _revision(row):
    cells = _children(row, "td")
    if len(cells) < 6:
        raise PageHistoryError("incomplete revision row")
    number_match = re.fullmatch(r"\s*(\d+)\.\s*", cells[0].text())
    if not number_match:
        raise PageHistoryError("invalid revision number")
    number = _positive(number_match.group(1), "revision number", allow_zero=True)
    row_id = re.fullmatch(r"revision-row-(\d+)", row.attrs.get("id") or "")
    if row_id is None:
        raise PageHistoryError("missing revision row ID")
    revision_id = _positive(row_id.group(1), "revision ID")
    actions = list(_walk(cells[3]))
    if _one(_handler_ids(actions, "showSource"), "source ID") != revision_id:
        raise PageHistoryError("source ID differs from revision row")
    versions = _handler_ids(actions, "showVersion")
    if versions and _optional_identity(versions, "version ID") != revision_id:
        raise PageHistoryError("version ID differs from revision row")
    user_container = [node for node in _walk(cells[4]) if _class(node, "printuser")]
    user_ids = _handler_ids(
        list(_walk(_one(user_container, "author container"))), "userInfo"
    )
    author_id = _optional_identity(user_ids, "author ID")
    timestamps = [node for node in _walk(cells[5]) if _class(node, "odate")]
    if len(timestamps) > 1:
        raise PageHistoryError("ambiguous revision date")
    created_at = None
    if timestamps:
        classes = (timestamps[0].attrs.get("class") or "").split()
        time_classes = [value for value in classes if value.startswith("time_")]
        created_at = _positive(
            _one(time_classes, "revision epoch")[5:], "revision epoch"
        )
    flags = [node for node in _walk(cells[2]) if _class(node, "spantip")]
    comments = (
        _one(_children(cells[5], "td"), "revision comment cell").text().strip() or None
    )
    return {
        "number": number,
        "revision_id": revision_id,
        "author_id": author_id,
        "created_at": created_at,
        "comments": comments,
        "flags": [
            {"marker": node.text().strip(), "description": node.attrs.get("title")}
            for node in flags
        ],
    }


def _pager(root):
    pager = _one([node for node in _walk(root) if _class(node, "pager")], "pager")
    page = _positive(
        _one([node for node in _walk(pager) if _class(node, "current")], "current page")
        .text()
        .strip(),
        "page",
    )
    links = {}
    for node in _walk(pager):
        if node.tag != "a":
            continue
        targets = _handler_ids([node], "updatePagedList")
        target = _one(targets, "pager target")
        label = node.text().strip()
        if "previous" in label:
            kind = "previous_page"
            if target != page - 1:
                raise PageHistoryError("invalid previous page")
        elif "next" in label:
            kind = "next_page"
            if target != page + 1:
                raise PageHistoryError("invalid next page")
        elif label.isdigit():
            if int(label) != target:
                raise PageHistoryError("mismatched pager target")
            continue
        else:
            raise PageHistoryError("unknown pager target")
        if kind in links:
            raise PageHistoryError("duplicate pager target")
        links[kind] = target
    return {
        "page": page,
        "previous_page": links.get("previous_page"),
        "next_page": links.get("next_page"),
    }


def parse_history_list(html):
    """Return sourced revision metadata and pager state for one native list page."""
    if not isinstance(html, str):
        raise PageHistoryError("history HTML must be text")
    parser = _ListingHTML()
    parser.feed(html)
    container = _one(
        [node for node in _walk(parser.root) if _class(node, "page-history")],
        "history container",
    )
    rows = [
        node for node in _walk(container) if node.tag == "tr" and _children(node, "td")
    ]
    if (
        rows
        and rows[0].attrs.get("id") is None
        and _children(rows[0], "td")[0].text().strip().lower() == "rev."
    ):
        rows = rows[1:]
    if not rows:
        raise PageHistoryError("history list has no revision rows")
    revisions = [_revision(row) for row in rows]
    _validate_revisions(revisions)
    return {"revisions": revisions, **_pager(parser.root)}


def _validate_revisions(revisions):
    numbers = [entry["number"] for entry in revisions]
    ids = [entry["revision_id"] for entry in revisions]
    if len(set(numbers)) != len(numbers) or len(set(ids)) != len(ids):
        raise PageHistoryError("duplicate revision number or ID")
    if numbers != sorted(numbers, reverse=True):
        raise PageHistoryError("revision numbers are not descending")


def parse_history_pages(html_pages):
    """Validate consecutive fetched list pages, retaining only observed revisions."""
    revisions = []
    previous = None
    for html in html_pages:
        result = parse_history_list(html)
        if previous is None:
            if result["page"] != 1 or result["previous_page"] is not None:
                raise PageHistoryError("history must begin at page 1")
        elif (
            result["page"] != previous["next_page"]
            or result["previous_page"] != previous["page"]
        ):
            raise PageHistoryError("missing or inconsistent history page")
        if revisions and revisions[-1]["number"] <= result["revisions"][0]["number"]:
            raise PageHistoryError("history page ordering is inconsistent")
        revisions.extend(result["revisions"])
        previous = result
    if previous is None or previous["next_page"] is not None:
        raise PageHistoryError("history pagination incomplete")
    _validate_revisions(revisions)
    unobserved = [
        (older["number"] - 1, newer["number"] + 1)
        for older, newer in zip(revisions, revisions[1:])
        if older["number"] - newer["number"] > 1
    ]
    if revisions[-1]["number"] > 0:
        unobserved.append((revisions[-1]["number"] - 1, 0))
    return {"revisions": revisions, "unobserved_ranges": unobserved}
