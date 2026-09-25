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
    title = source.get("title")
    fullname = source["fullname"]
    title_line = (
        f"Original title: {_literal_lines(title)}[br]"
        if title is not None
        else "Original title unavailable in metadata export[br]"
    )
    content = (
        "Source reference (not an executable template, style, or system page)[br]"
        f"{title_line}"
        f"Original fullname: {_literal_lines(fullname)}[br]"
        f"Source text:[br][code]{_literal_lines(raw_source)}[/code]"
    )
    tags = list(
        dict.fromkeys(
            [*source.get("tags", []), "source-reference", f"cobalt-source:{fullname}"]
        )
    )
    return {
        "title": f"Source reference: {fullname}",
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
