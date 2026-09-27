"""Apply reviewed current-page link rewrites without touching archived revisions."""

import hashlib
from urllib.parse import unquote_to_bytes, urlsplit

from .page_link_translation import translate_page_links
from .wikidot_sync import PAGE_SYNC_MARKER, import_owned


def _sha256(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _page(rpc, site_id, reference):
    return rpc.rpc(
        "page_get",
        {"site_id": site_id, "page": reference, "details": {"wikitext": True}},
    )


def _confirmed_destinations(rpc, site_id, source, source_origin, target_origin):
    candidates = translate_page_links(source, source_origin, target_origin, set())
    confirmed = set()
    for decision in candidates.decisions:
        if decision.status != "unconfirmed":
            continue
        slug = unquote_to_bytes(urlsplit(decision.url).path[1:]).decode("utf-8")
        destination = _page(rpc, site_id, slug)
        if destination is not None:
            if destination.get("slug") != slug or not isinstance(
                destination.get("page_id"), int
            ):
                raise ValueError("destination identity drift")
            confirmed.add(slug)
    return confirmed


def _prepare(rpc, site_id, user_id, source_origin, target_origin, entry):
    page_id = entry["page_id"]
    page = _page(rpc, site_id, page_id)
    if (
        entry["import_owned"] is not True
        or page is None
        or page.get("page_id") != page_id
        or not isinstance(page.get("slug"), str)
        or not page["slug"]
        or page.get("revision_id") != entry["revision_id"]
        or not import_owned(page, user_id, (PAGE_SYNC_MARKER,))
    ):
        raise ValueError(f"page {page_id}: reviewed revision or ownership drift")
    source = page.get("wikitext")
    if not isinstance(source, str) or _sha256(source) != entry["before_sha256"]:
        raise ValueError(f"page {page_id}: reviewed source drift")
    confirmed = _confirmed_destinations(
        rpc, site_id, source, source_origin, target_origin
    )
    result = translate_page_links(source, source_origin, target_origin, confirmed)
    rewritten = result.counts.get("rewritten", 0)
    if (
        rewritten <= 0
        or rewritten != entry["counts"]["rewritten"]
        or result.text == source
        or _sha256(result.text) != entry["after_sha256"]
    ):
        raise ValueError(f"page {page_id}: reviewed translation drift")
    return result.text, rewritten, page["slug"]


def apply_current_links(rpc, site_id, user_id, source_origin, target_origin, plan):
    """Preflight the complete reviewed plan, then revalidate each page before editing.

    ``rpc`` supplies ``rpc(method, params)``; no login, provisioning or network
    transport is created here. An exception stops the remaining writes.
    """
    entries = list(plan)
    if len({entry["page_id"] for entry in entries}) != len(entries):
        raise ValueError("duplicate reviewed page")
    preflight = [
        _prepare(rpc, site_id, user_id, source_origin, target_origin, entry)
        for entry in entries
    ]
    receipts = []
    for entry, (_, _, slug) in zip(entries, preflight):
        text, rewritten, current_slug = _prepare(
            rpc, site_id, user_id, source_origin, target_origin, entry
        )
        if slug != current_slug:
            raise ValueError(f"page {entry['page_id']}: slug drift")
        rpc.rpc(
            "page_edit",
            {
                "site_id": site_id,
                "user_id": user_id,
                "page": entry["page_id"],
                "last_revision_id": entry["revision_id"],
                "wikitext": text,
                "revision_comments": "Cobalt page-link migration "
                + entry["before_sha256"],
                "preserve_draft": True,
                "do_not_notify_watchers": True,
                "ip_address": "127.0.0.1",
            },
        )
        edited = _page(rpc, site_id, entry["page_id"])
        if (
            edited is None
            or edited.get("page_id") != entry["page_id"]
            or edited.get("slug") != slug
            or not isinstance(edited.get("wikitext"), str)
            or _sha256(edited["wikitext"]) != entry["after_sha256"]
            or not isinstance(edited.get("revision_id"), int)
            or edited["revision_id"] <= entry["revision_id"]
        ):
            raise ValueError(f"page {entry['page_id']}: edited page readback drift")
        receipts.append(
            {
                "page_id": entry["page_id"],
                "before_revision_id": entry["revision_id"],
                "after_revision_id": edited["revision_id"],
                "before_sha256": entry["before_sha256"],
                "after_sha256": entry["after_sha256"],
                "rewritten": rewritten,
            }
        )
    return receipts
