"""Pure payload for private, non-executable Wikidot source references."""

from html import escape


def _literal_lines(value: str) -> str:
    if "[/noparse]" in value.lower():
        raise ValueError("literal source contains a noparse closing tag")
    return "[br]".join(
        f"[noparse]{escape(line)}[/noparse]" if line else ""
        for line in value.split("\n")
    )


def reference_payload(source, raw_source):
    """Represent source as text in a private World Anvil article payload."""
    title = source["title"]
    fullname = source["fullname"]
    content = (
        "Source reference (not an executable template, style, or system page)[br]"
        f"Original title: {_literal_lines(title)}[br]"
        f"Original fullname: {_literal_lines(fullname)}[br]"
        f"Source text:[br]{_literal_lines(raw_source)}"
    )
    tags = list(
        dict.fromkeys(
            [*source.get("tags", []), "source-reference", f"cobalt-source:{fullname}"]
        )
    )
    return {
        "title": f"Source reference: {escape(title)}",
        "templateType": "article",
        "state": "private",
        "isDraft": False,
        "isWip": True,
        "editor": "plutarch",
        "tags": ",".join(tags),
        "content": content,
        "displayTitle": True,
        "displayAuthor": False,
    }
