"""Extract allowlisted Wikidot page metadata without running JavaScript or doing I/O."""

from dataclasses import dataclass
from html.parser import HTMLParser
import json
import re


class PageMetadataError(ValueError):
    """Page HTML does not establish a consistent metadata record."""


class SourcePageUnavailable(PageMetadataError):
    """The source rendered a denial or missing-page response, regardless of HTTP status."""

    def __init__(self, reason: str):
        self.reason = reason
        super().__init__(f"source page unavailable: {reason}")


_VOID_TAGS = frozenset(
    "area base br col embed hr img input link meta param source track wbr".split()
)
_INERT_TAGS = frozenset({"script", "style", "template"})
_TEXT_BREAKS = frozenset({"br", "div", "p", "h1", "h2", "h3", "li"})
_SCRIPT_TYPES = frozenset({"", "text/javascript", "application/javascript", "module"})
_SECTIONS = frozenset({"page-title", "page-info", "page-content"})
_ASSIGNMENT = re.compile(
    r"(?<![\w$.])WIKIREQUEST\s*\.\s*info\s*\.\s*"
    r"(?P<field>pageUnixName|pageId)\s*=(?!=|>)"
)
_REVISION = re.compile(r"\bpage\s+revision\s*:\s*([^\s,;]+)", re.IGNORECASE)
_HEX = re.compile(r"[0-9a-fA-F]+")
_ESCAPES = {
    "'": "'",
    '"': '"',
    "\\": "\\",
    "/": "/",
    "b": "\b",
    "f": "\f",
    "n": "\n",
    "r": "\r",
    "t": "\t",
    "v": "\v",
}


@dataclass
class _Frame:
    tag: str
    sections: frozenset[str]
    inert: bool
    anchor: list[str] | None = None
    script: list[str] | None = None


class _PageHTML(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.frames: list[_Frame] = []
        self.counts: dict[str, int] = {}
        self.text: dict[str, list[str]] = {name: [] for name in _SECTIONS}
        self.tags: list[list[str]] = []
        self.timestamps: list[str] = []
        self.scripts: list[list[str]] = []

    def handle_starttag(self, tag, attrs):
        attributes = dict(attrs)
        parent = self.frames[-1] if self.frames else None
        parent_inert = parent.inert if parent else False
        inert = parent_inert or tag in _INERT_TAGS
        sections = parent.sections if parent else frozenset()
        anchor = parent.anchor if parent else None
        if not inert:
            sections = self._open_sections(sections, attributes)
            anchor = self._open_anchor(tag, sections, anchor)
            self._collect_timestamp(sections, attributes)
        script = self._open_script(tag, attributes, parent_inert)
        frame = _Frame(tag, sections, inert, anchor, script)
        if not inert and tag in _TEXT_BREAKS:
            self._append_text(frame, " ")
        if tag not in _VOID_TAGS:
            self.frames.append(frame)

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in _VOID_TAGS:
            self.handle_endtag(tag)

    def handle_endtag(self, tag):
        for index in range(len(self.frames) - 1, -1, -1):
            if self.frames[index].tag == tag:
                frame = self.frames[index]
                if not frame.inert and tag in _TEXT_BREAKS:
                    self._append_text(frame, " ")
                del self.frames[index:]
                return

    def handle_data(self, data):
        if not self.frames:
            return
        frame = self.frames[-1]
        if frame.script is not None:
            frame.script.append(data)
        if not frame.inert:
            self._append_text(frame, data)

    def _open_sections(self, inherited, attributes):
        sections = set(inherited)
        names = []
        identifier = attributes.get("id")
        if identifier in _SECTIONS:
            names.append(identifier)
        if "page-tags" in (attributes.get("class") or "").split():
            names.append("page-tags")
        for name in names:
            self.counts[name] = self.counts.get(name, 0) + 1
            sections.add(name)
        return frozenset(sections)

    def _open_anchor(self, tag, sections, inherited):
        if tag != "a" or "page-tags" not in sections:
            return inherited
        if inherited is not None:
            raise PageMetadataError("nested tag anchors are inconsistent")
        anchor: list[str] = []
        self.tags.append(anchor)
        return anchor

    def _collect_timestamp(self, sections, attributes):
        classes = (attributes.get("class") or "").split()
        if "page-info" in sections and "odate" in classes:
            tokens = [value[5:] for value in classes if value.startswith("time_")]
            if len(tokens) != 1:
                raise PageMetadataError("footer odate requires one time_<epoch> class")
            self.timestamps.append(tokens[0])

    def _open_script(self, tag, attributes, parent_inert):
        script_type = (attributes.get("type") or "").strip().lower()
        if tag != "script" or parent_inert or "src" in attributes:
            return None
        if script_type not in _SCRIPT_TYPES:
            return None
        script: list[str] = []
        self.scripts.append(script)
        return script

    def _append_text(self, frame, data):
        for section in frame.sections:
            if section in self.text:
                self.text[section].append(data)
        if frame.anchor is not None:
            frame.anchor.append(data)


def _compact_text(chunks: list[str]) -> str:
    return " ".join("".join(chunks).split())


def _reject_unavailable_page(parsed: _PageHTML) -> None:
    title = _compact_text(parsed.text["page-title"]).casefold()
    content = _compact_text(parsed.text["page-content"]).casefold()
    if title in {"private content", "access denied", "permission denied"}:
        raise SourcePageUnavailable("denied")
    if content.startswith("this area of the site is private"):
        raise SourcePageUnavailable("denied")
    if title in {"page not found", "page does not exist"}:
        raise SourcePageUnavailable("not_found")
    if content.startswith("the page you want to access does not exist"):
        raise SourcePageUnavailable("not_found")


def _quoted_end(script: str, start: int) -> int:
    quote = script[start]
    position = start + 1
    while position < len(script):
        character = script[position]
        if character == "\\":
            position += 2
        elif character == quote:
            return position + 1
        else:
            position += 1
    raise PageMetadataError("unterminated script string literal")


def _single_quoted_string(literal: str) -> str:
    decoded = []
    position = 1
    while position < len(literal) - 1:
        character = literal[position]
        position += 1
        if character != "\\":
            decoded.append(character)
            continue
        escape = literal[position]
        position += 1
        if escape in _ESCAPES:
            decoded.append(_ESCAPES[escape])
            continue
        if escape not in {"u", "x"}:
            raise PageMetadataError("unsupported pageUnixName string escape")
        width = 4 if escape == "u" else 2
        digits = literal[position : position + width]
        if len(digits) != width or _HEX.fullmatch(digits) is None:
            raise PageMetadataError("invalid pageUnixName hexadecimal escape")
        decoded.append(chr(int(digits, 16)))
        position += width
    return "".join(decoded)


def _decode_fullname(literal: str) -> str:
    try:
        value = (
            json.loads(literal) if literal[0] == '"' else _single_quoted_string(literal)
        )
        # Combine JS UTF-16 surrogate pairs, rejecting isolated surrogate codepoints.
        value = value.encode("utf-16-le", "surrogatepass").decode("utf-16-le")
    except (ValueError, UnicodeError):
        raise PageMetadataError("invalid pageUnixName string literal") from None
    if not value or any(
        character.isspace() or ord(character) < 32 for character in value
    ):
        raise PageMetadataError(
            "pageUnixName must be a nonempty fullname without whitespace"
        )
    return value


def _decimal_integer(value: str, field: str, *, positive=False) -> int:
    pattern = r"[1-9][0-9]*" if positive else r"(?:0|[1-9][0-9]*)"
    if re.fullmatch(pattern, value) is None:
        raise PageMetadataError(f"invalid {field}: expected a decimal integer")
    try:
        return int(value)
    except ValueError:
        raise PageMetadataError(
            f"invalid {field}: decimal integer cannot be represented"
        ) from None


def _read_assignment(script: str, start: int, field: str) -> tuple[str | int, int]:
    position = start
    while position < len(script) and script[position].isspace():
        position += 1
    if field == "pageUnixName":
        if position == len(script) or script[position] not in "\"'":
            raise PageMetadataError("pageUnixName requires a quoted scalar literal")
        end = _quoted_end(script, position)
        value = _decode_fullname(script[position:end])
    else:
        match = re.match(r"[0-9]+", script[position:])
        if match is None:
            raise PageMetadataError(
                "pageId requires a positive decimal integer literal"
            )
        end = position + match.end()
        value = _decimal_integer(match.group(), "pageId", positive=True)
    remainder = script[end:]
    boundary = re.match(r"[ \t\r\n]*(?:;|$)", remainder)
    if boundary is None:
        raise PageMetadataError(
            f"unsupported {field} assignment syntax; expected literal and semicolon"
        )
    return value, end + boundary.end()


def _skip_comment(script: str, position: int) -> int:
    if script.startswith("//", position):
        end = script.find("\n", position + 2)
        return len(script) if end == -1 else end + 1
    end = script.find("*/", position + 2)
    if end == -1:
        raise PageMetadataError("unterminated script comment containing metadata")
    return end + 2


def _script_assignments(script: str):
    if _ASSIGNMENT.search(script) is None:
        return
    position = 0
    while position < len(script):
        if script.startswith(("//", "/*"), position):
            position = _skip_comment(script, position)
        elif script[position] in "\"'`":
            position = _quoted_end(script, position)
        else:
            match = _ASSIGNMENT.match(script, position)
            if match is None:
                position += 1
                continue
            field = match.group("field")
            value, position = _read_assignment(script, match.end(), field)
            yield field, value


def _identity(parsed: _PageHTML) -> dict:
    scalars = {}
    for chunks in parsed.scripts:
        for field, value in _script_assignments("".join(chunks)):
            if field in scalars and scalars[field] != value:
                raise PageMetadataError(f"conflicting {field} assignments")
            scalars[field] = value
    for field in ("pageUnixName", "pageId"):
        if field not in scalars:
            raise PageMetadataError(f"missing WIKIREQUEST.info.{field} assignment")
    return scalars


def _visible_metadata(parsed: _PageHTML) -> dict:
    for section in ("page-title", "page-tags", "page-info"):
        if parsed.counts.get(section, 0) != 1:
            raise PageMetadataError(
                f"expected exactly one {section} metadata container"
            )
    title = _compact_text(parsed.text["page-title"])
    if not title:
        raise PageMetadataError("page-title is empty")
    tags = ["".join(chunks).strip() for chunks in parsed.tags]
    if any(not tag for tag in tags):
        raise PageMetadataError("page-tags contains an empty tag")
    revisions = _REVISION.findall(_compact_text(parsed.text["page-info"]))
    if len(revisions) != 1:
        raise PageMetadataError("expected exactly one current page revision in footer")
    if len(parsed.timestamps) != 1:
        raise PageMetadataError("expected exactly one updated_at timestamp in footer")
    return {
        "title": title,
        "tags": sorted(set(tags)),
        "revision_number": _decimal_integer(revisions[0], "revision_number"),
        "updated_at": _decimal_integer(parsed.timestamps[0], "updated_at"),
    }


def parse_page_metadata(html: str, expected_fullname: str | None = None) -> dict:
    """Parse one source response; errors never include source text or scalar values.

    Only literal WIKIREQUEST assignments and scoped HTML metadata are supported.
    Missing or ambiguous metadata is an error, not an empty/default record.
    """
    parsed = _PageHTML()
    parsed.feed(html)
    parsed.close()
    _reject_unavailable_page(parsed)
    identity = _identity(parsed)
    if expected_fullname is not None and identity["pageUnixName"] != expected_fullname:
        raise PageMetadataError("canonical fullname does not match expected_fullname")
    return {
        "fullname": identity["pageUnixName"],
        "page_id": identity["pageId"],
        **_visible_metadata(parsed),
    }
