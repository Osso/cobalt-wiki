"""Conservative, pure translation of confirmed Wikidot page URL destinations."""

from collections import Counter
from dataclasses import dataclass
import re
from urllib.parse import unquote_to_bytes, urlsplit


URL = re.compile(r"https?://[^\s<>\"|\[\]]+")
BLOCK = re.compile(r"\[\[(code|html|css|module\s+css)\b[^\]\n]*\]\]", re.I)
HTML = re.compile(r"<([a-z][\w:-]*)\b[^>]*>", re.I)
BAD_PERCENT = re.compile(r"%(?![0-9a-fA-F]{2})")
TRAILING = ",.;!?) }"


@dataclass(frozen=True)
class LinkDecision:
    url: str
    status: str
    replacement: str | None = None


@dataclass(frozen=True)
class TranslationResult:
    text: str
    decisions: tuple[LinkDecision, ...]
    counts: dict[str, int]


def _opaque_end(source: str, pos: int) -> int | None:
    """Return end of a known literal region, or an unsupported bracket construct."""
    for opening, closing in (
        ("[!--", "--]"),
        ("<!--", "-->"),
        ("@@", "@@"),
        ("@<", ">@"),
        ("{{", "}}"),
        ("`", "`"),
    ):
        if source.startswith(opening, pos):
            end = source.find(closing, pos + len(opening))
            return len(source) if end < 0 else end + len(closing)

    block = BLOCK.match(source, pos)
    if block:
        name = block.group(1).split()[0]
        close = re.search(r"\[\[/" + name + r"\]\]", source[block.end() :], re.I)
        return len(source) if close is None else block.end() + close.end()

    tag = HTML.match(source, pos)
    if tag:
        if source[tag.start() : tag.end()].endswith("/>"):
            return tag.end()
        closing = re.search(
            r"</" + re.escape(tag.group(1)) + r"\s*>", source[tag.end() :], re.I
        )
        return len(source) if closing is None else tag.end() + closing.end()

    if source.startswith("[[", pos):
        end = source.find("]]", pos + 2)
        return len(source) if end < 0 else end + 2
    if source.startswith("[", pos):
        end = source.find("]", pos + 1)
        return len(source) if end < 0 else end + 1
    return None


def _destination(source: str, pos: int) -> tuple[int, int, int] | None:
    """Recognize one link and return its end and destination boundaries."""
    if source.startswith("[[[", pos):
        prefix = 4 if source.startswith("[[[*", pos) else 3
        end = source.find("]]]", pos + prefix)
        if end < 0 or "\n" in source[pos:end]:
            return None
        target_end = source.find("|", pos + prefix, end)
        if target_end < 0:
            target_end = end
        target = source[pos + prefix : target_end]
        lead = len(target) - len(target.lstrip())
        return (
            end + 3,
            pos + prefix + lead,
            target_end - len(target) + len(target.rstrip()),
        )
    if source.startswith("[", pos) and not source.startswith("[[", pos):
        prefix = 2 if source.startswith("[*", pos) else 1
        end = source.find("]", pos + prefix)
        if end < 0 or "\n" in source[pos:end]:
            return None
        match = URL.match(source, pos + prefix)
        if (
            match
            and match.end() <= end
            and (match.end() == end or source[match.end()].isspace())
        ):
            return end + 1, match.start(), match.end()
    return None


def translate_page_links(
    source: str,
    source_origin: str,
    target_origin: str,
    confirmed_pages: set[str] | dict[str, str],
) -> TranslationResult:
    """Rewrite URL destinations only; no storage or network access.

    A mapping translates exact decoded source page names to confirmed target names.
    A set retains the original encoded path. Unknown syntax is left untouched.
    """
    origin = urlsplit(source_origin)
    target = urlsplit(target_origin)
    if (
        origin.scheme not in ("http", "https")
        or not origin.netloc
        or target.scheme not in ("http", "https")
        or not target.netloc
        or any(
            (
                origin.path,
                origin.query,
                origin.fragment,
                target.path,
                target.query,
                target.fragment,
            )
        )
        or "@" in origin.netloc
        or ":" in origin.netloc
        or "@" in target.netloc
        or ":" in target.netloc
    ):
        raise ValueError("origins must be bare http(s) hosts without ports or userinfo")

    decisions: list[LinkDecision] = []
    output: list[str] = []
    position = 0

    def decide(url: str, context: str) -> str:
        if context != "destination":
            decisions.append(LinkDecision(url, "unsafe_context"))
            return url
        parts = urlsplit(url)
        if parts.scheme not in ("http", "https") or parts.netloc != origin.netloc:
            decisions.append(LinkDecision(url, "external"))
            return url
        path = parts.path
        if not path.startswith("/") or BAD_PERCENT.search(path):
            decisions.append(LinkDecision(url, "non_page"))
            return url
        try:
            name = unquote_to_bytes(path[1:]).decode("utf-8", "strict")
        except UnicodeDecodeError:
            decisions.append(LinkDecision(url, "non_page"))
            return url
        if not name or "/" in name or "\\" in name:
            decisions.append(LinkDecision(url, "non_page"))
            return url
        if name not in confirmed_pages:
            decisions.append(LinkDecision(url, "unconfirmed"))
            return url
        if isinstance(confirmed_pages, dict):
            translated = confirmed_pages[name]
            if not translated or "/" in translated or "\\" in translated:
                raise ValueError("target page must be one canonical component")
            from urllib.parse import quote

            path = "/" + quote(translated, safe=":-._~")
        replacement = (
            target_origin
            + path
            + url[len(parts.scheme) + 3 + len(parts.netloc) + len(parts.path) :]
        )
        decisions.append(LinkDecision(url, "rewritten", replacement))
        return replacement

    def render_segment(start: int, end: int, context: str) -> str:
        segment = source[start:end]
        if context != "destination":
            for match in URL.finditer(segment):
                decide(match.group(), context)
            return segment
        match = URL.fullmatch(segment)
        return decide(segment, context) if match else segment

    while position < len(source):
        link = (
            _destination(source, position)
            if source[position] == "["
            and (position == 0 or source[position - 1] != "\\")
            else None
        )
        if link:
            end, start, stop = link
            output.extend(
                (
                    source[position:start],
                    render_segment(start, stop, "destination"),
                    render_segment(stop, end, "unsafe"),
                )
            )
            position = end
            continue
        opaque = _opaque_end(source, position)
        if opaque is not None:
            output.append(render_segment(position, opaque, "unsafe"))
            position = opaque
            continue
        match = URL.match(source, position)
        if match:
            end = match.end()
            while end > position and source[end - 1] in TRAILING:
                end -= 1
            if end > position:
                context = (
                    "unsafe"
                    if position and source[position - 1] == "\\"
                    else "destination"
                )
                output.append(render_segment(position, end, context))
                position = end
                continue
        output.append(source[position])
        position += 1

    counts = dict(Counter(decision.status for decision in decisions))
    return TranslationResult("".join(output), tuple(decisions), counts)
