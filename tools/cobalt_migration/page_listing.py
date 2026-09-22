"""Parse Wikidot listing HTML without executing scripts or performing I/O."""

from dataclasses import dataclass, field
from html.parser import HTMLParser
import re
from urllib.parse import unquote, urljoin, urlsplit

from .page_metadata import SourcePageUnavailable


class PageListingError(ValueError):
    """Listing HTML cannot establish page identities or pagination."""


_VOID = frozenset(
    "area base br col embed hr img input link meta param source track wbr".split()
)
_INERT = frozenset({"script", "style", "template"})


@dataclass
class _Node:
    tag: str
    attrs: dict
    children: list = field(default_factory=list)

    def text(self):
        return "".join(
            child if isinstance(child, str) else child.text() for child in self.children
        )


class _ListingHTML(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.root = _Node("", {})
        self.stack = [self.root]

    def handle_starttag(self, tag, attrs):
        node = _Node(tag, dict(attrs))
        self.stack[-1].children.append(node)
        if tag not in _VOID:
            self.stack.append(node)

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in _VOID:
            self.handle_endtag(tag)

    def handle_endtag(self, tag):
        for index in range(len(self.stack) - 1, 0, -1):
            if self.stack[index].tag == tag:
                del self.stack[index:]
                break

    def handle_data(self, data):
        if not any(node.tag in _INERT for node in self.stack):
            self.stack[-1].children.append(data)


def _walk(node):
    if node.tag in _INERT:
        return
    yield node
    for child in node.children:
        if isinstance(child, _Node):
            yield from _walk(child)


def _has_class(node, name):
    return name in (node.attrs.get("class") or "").split()


def _origin(parts):
    try:
        if (
            parts.scheme not in {"http", "https"}
            or not parts.hostname
            or parts.username is not None
            or parts.password is not None
        ):
            raise ValueError
        return (
            parts.scheme,
            parts.hostname.lower(),
            parts.port or (443 if parts.scheme == "https" else 80),
        )
    except ValueError:
        raise PageListingError(
            "expected an HTTP(S) origin without credentials"
        ) from None


def _same_origin_url(href, source_origin, origin):
    if not href or href.startswith(("#", "?")):
        return None
    try:
        parts = urlsplit(urljoin(source_origin + "/pagelist", href))
        if _origin(parts) != origin:
            return None
    except ValueError:
        return None
    return parts


def _fullname(path):
    if not path.startswith("/") or "/" in path[1:] or path == "/":
        return None
    if re.search(r"%(?![0-9a-fA-F]{2})", path):
        raise PageListingError("malformed percent encoding in page link")
    try:
        name = unquote(path[1:], encoding="utf-8", errors="strict")
    except UnicodeError:
        raise PageListingError("page link is not valid UTF-8") from None
    if any(c.isspace() or ord(c) < 32 or ord(c) == 127 or c in "/\\" for c in name):
        raise PageListingError(
            "page link contains an ambiguous separator or whitespace"
        )
    return name


def _reject_unavailable(nodes):
    for node in nodes:
        identifier = node.attrs.get("id")
        if identifier not in {"page-title", "page-content"}:
            continue
        text = " ".join(node.text().split()).casefold()
        if identifier == "page-title":
            if text in {"private content", "access denied", "permission denied"}:
                raise SourcePageUnavailable("denied")
            if text in {"page not found", "page does not exist"}:
                raise SourcePageUnavailable("not_found")
        elif text.startswith("this area of the site is private"):
            raise SourcePageUnavailable("denied")
        elif text.startswith("the page you want to access does not exist"):
            raise SourcePageUnavailable("not_found")


def _collect_block(node, source_origin, origin, names, pages, in_pager=False):
    if node.tag in _INERT:
        return
    in_pager = in_pager or _has_class(node, "pager")
    if _has_class(node, "pager-no"):
        pages.extend(int(value) for value in re.findall(r"\b[0-9]+\b", node.text()))
    if node.tag == "a":
        parts = _same_origin_url(node.attrs.get("href"), source_origin, origin)
        if parts is not None:
            if in_pager:
                match = re.fullmatch(r"/pagelist/p/([1-9][0-9]*)", parts.path)
                if match:
                    pages.append(int(match[1]))
            elif " ".join(node.text().split()).casefold() != "edit":
                name = _fullname(parts.path)
                if name is not None:
                    names.setdefault(name, None)
    for child in node.children:
        if isinstance(child, _Node):
            _collect_block(child, source_origin, origin, names, pages, in_pager)


def parse_page_listing(html: str, source_origin: str) -> dict:
    """Return ordered unique ``fullnames`` and ``highest_page`` (at least one).

    URL paths are decoded exactly once as strict UTF-8. No slug normalization
    or archive-name mapping is performed. Repeats across responses remain the
    caller's responsibility.
    """
    try:
        source = urlsplit(source_origin)
    except ValueError:
        raise PageListingError("invalid source origin") from None
    origin = _origin(source)
    if source.path not in {"", "/"} or source.query or source.fragment:
        raise PageListingError(
            "source_origin must not contain a path, query or fragment"
        )
    source_origin = source_origin.rstrip("/")
    parser = _ListingHTML()
    parser.feed(html)
    parser.close()
    nodes = list(_walk(parser.root))
    _reject_unavailable(nodes)
    content = [node for node in nodes if node.attrs.get("id") == "page-content"]
    if len(content) != 1:
        raise PageListingError("expected exactly one page-content element")
    blocks = [node for node in _walk(content[0]) if _has_class(node, "list-pages-box")]
    if not blocks:
        raise PageListingError("page-content has no list-pages-box blocks")
    names = {}
    pages = [1]
    for block in blocks:
        _collect_block(block, source_origin, origin, names, pages)
    return {"fullnames": list(names), "highest_page": max(pages)}
