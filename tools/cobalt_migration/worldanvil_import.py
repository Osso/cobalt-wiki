"""Additive import primitives. Unsupported source content blocks, never disappears."""

import hashlib
import json
import re
import unicodedata
from pathlib import Path
from urllib.parse import urlsplit

from .archive import write_manifest


class ImportBlocked(ValueError):
    """A page needs source, identity, conversion, or remote-result reconciliation."""


PLAYER_FIELDS = {
    "portrait",
    "nicknames",
    "pronouns",
    "battleTag",
    "discordUsername",
    "timezone",
    "whoAmI",
    "rpPrefs",
    "contactPrefs",
}


def _text(value):
    if value is None or value == "@@":
        return ""
    if not isinstance(value, str):
        raise ImportBlocked("profile field is not text")
    return value


def _plain(value):
    value = _text(value)
    without_urls = re.sub(r"https?://[^\s<>\[\]]+", "URL", value)
    if re.search(
        r"\[|\]|\*\*|//|__|\|\||@@|^\s*[+*>#]|^\s*[-=]{3,}", without_urls, re.MULTILINE
    ):
        raise ImportBlocked("wiki formatting needs conversion before import")
    return value


def _paragraphs(value):
    return "\n".join(
        "[p]" + paragraph.replace("\n", "[br]") + "[/p]"
        for paragraph in re.split(r"\n\s*\n", _plain(value).strip())
        if paragraph
    )


def _portrait_markup(original, portrait):
    if not original:
        if portrait is not None:
            raise ImportBlocked("portrait reference supplied without a source portrait")
        return None
    if portrait is None:
        raise ImportBlocked("portrait must be migrated before creating this profile")
    if type(portrait) is int and portrait > 0:
        return f"[img:{portrait}|none]"
    if isinstance(portrait, str) and portrait == original:
        if re.search(r"[\s|\[\]]", portrait):
            raise ImportBlocked("portrait URL contains BBCode delimiters or whitespace")
        parsed = urlsplit(portrait)
        if parsed.scheme in ("http", "https") and parsed.netloc:
            return f"[img:{portrait}|none]"
    raise ImportBlocked(
        "portrait reference must be a positive image ID or matching http(s) URL"
    )


def player_payload(source, fields, *, portrait=None):
    """Convert player forms with only explicitly resolved portrait references."""
    if not source["fullname"].startswith("player:"):
        raise ImportBlocked("not a player profile")
    if not isinstance(fields, dict):
        raise ImportBlocked("player source is not a field mapping")
    unknown = [key for key in fields if key not in PLAYER_FIELDS and _text(fields[key])]
    if unknown:
        raise ImportBlocked("unhandled player fields: " + ", ".join(sorted(unknown)))
    image = _portrait_markup(_text(fields.get("portrait")), portrait)
    sections = [
        ("Who Am I?", "whoAmI"),
        ("RP Preferences", "rpPrefs"),
        ("Contact Preferences", "contactPrefs"),
    ]
    content = "\n".join(
        f"[h1]{label}[/h1]\n{_paragraphs(fields.get(key))}"
        for label, key in sections
        if _text(fields.get(key))
    )
    sidebar = [image] if image else []
    for label, key in (
        ("Nicknames", "nicknames"),
        ("Pronouns", "pronouns"),
        ("BattleTag", "battleTag"),
        ("Discord", "discordUsername"),
        ("Time Zone", "timezone"),
    ):
        value = _plain(fields.get(key))
        if "--" in value or "::" in value:
            raise ImportBlocked("sidebar definition delimiters need escaping")
        if value:
            sidebar.append(f"--{label}::{value}--")
    tags = list(
        dict.fromkeys(
            [*source.get("tags", []), "player", "cobalt-source:" + source["fullname"]]
        )
    )
    return {
        "title": source["title"],
        "templateType": "article",
        "state": "private",
        "isDraft": False,
        "isWip": True,
        "editor": "plutarch",
        "tags": ",".join(tags),
        "content": content,
        "sidepanelcontenttop": "\n".join(sidebar),
        "displayTitle": True,
        "displayAuthor": False,
        "displaySidebar": True,
    }


def _normalize(value):
    return "".join(
        c for c in unicodedata.normalize("NFKC", value or "").casefold() if c.isalnum()
    )


def _index_article(inventory, article):
    article_id = article["id"]
    inventory[("id", article_id)] = {article_id: article}
    for value in (article.get("title"), article.get("slug")):
        name = _normalize(value)
        if name:
            inventory.setdefault(("name", name), {})[article_id] = article
    for tag in (article.get("tags") or "").split(","):
        if tag.startswith("cobalt-source:"):
            inventory.setdefault(("marker", tag), {})[article_id] = article


def index_articles(articles):
    """Index a caller-owned remote inventory snapshot by name, source tag, and ID."""
    inventory = {}
    for article in articles:
        _index_article(inventory, article)
    return inventory


def _find_existing(source, inventory, title):
    names = {_normalize(title)}
    namespace, _, shortname = source["fullname"].partition(":")
    if namespace in {"player", "character", "bgc"}:
        names.add(_normalize(shortname))
    matches = {}
    for name in names - {""}:
        matches.update(inventory.get(("name", name), {}))
    marker = "cobalt-source:" + source["fullname"]
    matches.update(inventory.get(("marker", marker), {}))
    return list(matches.values())


def _load_journal(path, world_id):
    journal = (
        json.loads(path.read_text())
        if path.exists()
        else {"world_id": world_id, "pages": {}}
    )
    if journal.get("world_id") != world_id or not isinstance(
        journal.get("pages"), dict
    ):
        raise ImportBlocked("journal does not belong to this world")
    return journal


def _verify_created(client, world_id, article_id, payload):
    actual = client.get_article(article_id)
    if actual.get("world", {}).get("id") != world_id:
        raise ImportBlocked("created article readback world mismatch")
    for field in (
        "title",
        "templateType",
        "state",
        "isDraft",
        "content",
        "authornotes",
        "sidepanelcontenttop",
    ):
        if field in payload and actual.get(field) != payload[field]:
            raise ImportBlocked(f"created article readback mismatch: {field}")
    if set((actual.get("tags") or "").split(",")) != set(
        payload.get("tags", "").split(",")
    ):
        raise ImportBlocked("created article readback mismatch: tags")
    return actual


def import_page(client, world_id, source, payload, journal_path, inventory):
    """Create once against a caller-owned batch inventory; never retry uncertain creates."""
    path = Path(journal_path)
    journal = _load_journal(path, world_id)
    fullname = source["fullname"]
    fingerprint = hashlib.sha256(
        json.dumps(payload, sort_keys=True).encode()
    ).hexdigest()
    previous = journal["pages"].get(fullname)
    if previous:
        if previous.get("payload_sha256") != fingerprint:
            raise ImportBlocked("payload changed since recorded import")
        if previous["status"] not in ("created", "existing"):
            raise ImportBlocked(
                "prior creation needs reconciliation; refusing another create"
            )
        return previous
    existing = _find_existing(source, inventory, payload["title"])
    if len(existing) > 1:
        raise ImportBlocked("multiple live identity candidates; refusing creation")
    entry = {
        "status": "existing" if existing else "pending",
        "payload_sha256": fingerprint,
    }
    if existing:
        entry["id"] = existing[0]["id"]
    journal["pages"][fullname] = entry
    write_manifest(journal, path)
    if existing:
        return entry
    created = client.create_article(world_id, payload)
    entry.update(status="created_unverified", id=created["id"])
    write_manifest(journal, path)
    _index_article(inventory, {**payload, "id": created["id"]})
    _verify_created(client, world_id, created["id"], payload)
    entry["status"] = "created"
    write_manifest(journal, path)
    return entry
