"""Convert a source-rendered page-content HTML subtree into World Anvil BBCode."""

import re
from collections.abc import Callable
from dataclasses import dataclass, field
from html.parser import HTMLParser
from urllib.parse import urljoin, urlsplit


class UnsupportedContent(ValueError):
    """Content cannot be represented without a known loss or unsafe markup."""


@dataclass
class _Element:
    tag: str
    attrs: dict[str, str | None]
    children: list["_Element | str"] = field(default_factory=list)


_VOID = {
    "area",
    "base",
    "br",
    "col",
    "embed",
    "hr",
    "img",
    "input",
    "link",
    "meta",
    "param",
    "source",
    "track",
    "wbr",
}
_ALLOWED = {
    "div",
    "span",
    "p",
    "br",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "strong",
    "b",
    "em",
    "i",
    "u",
    "s",
    "strike",
    "del",
    "blockquote",
    "ul",
    "ol",
    "li",
    "hr",
    "table",
    "tbody",
    "thead",
    "tfoot",
    "tr",
    "td",
    "th",
    "pre",
    "code",
    "a",
    "img",
    "script",
    "iframe",
}
_BLOCK = {
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "blockquote",
    "ul",
    "ol",
    "hr",
    "table",
    "pre",
    "div",
    "iframe",
}
_MARKUP = {"strong": "b", "em": "i", "strike": "s", "del": "s"}
_TAG_ACTIONS = {
    "WIKIDOT.page.listeners.editTags(event)",
    "WIKIDOT.page.listeners.updateTagsByButton(event, '+_completed -@@')",
}
_YOUTUBE_EMBED = re.compile(
    r"https://www\.youtube\.com/embed/[A-Za-z0-9_-]{11}(?:\?si=[A-Za-z0-9_-]+)?"
)
_IFRAME_ATTRS = {
    "src",
    "title",
    "width",
    "height",
    "frameborder",
    "allow",
    "allowfullscreen",
}


class _PageParser(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.root: _Element | None = None
        self.stack: list[_Element] = []
        self.count = 0

    def handle_starttag(self, tag, attrs):
        attributes = dict(attrs)
        if attributes.get("id") == "page-content":
            self.count += 1
            if self.count > 1 or tag != "div":
                raise UnsupportedContent("expected exactly one div#page-content")
            self.root = _Element(tag, attributes)
            self.stack.append(self.root)
            return
        if not self.stack:
            return
        self._append(tag, attributes, tag in _VOID)

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in _VOID and self.stack:
            self.handle_endtag(tag)

    def _append(self, tag, attrs, void):
        if tag not in _ALLOWED:
            raise UnsupportedContent(f"unsupported element <{tag}>")
        if any(name.startswith("on") for name in attrs) and not (
            tag == "a"
            and set(attrs) == {"class", "href", "onclick"}
            and attrs["class"] == "wiki-standalone-button"
            and attrs["href"] == "javascript:;"
            and attrs["onclick"] in _TAG_ACTIONS
        ):
            raise UnsupportedContent(f"actionable attribute in <{tag}>")
        style = attrs.get("style") or ""
        hidden = (
            "hidden" in attrs
            or attrs.get("aria-hidden") == "true"
            or re.search(
                r"(?:^|;)\s*(?:display\s*:\s*none|visibility\s*:\s*hidden)\b",
                style,
                re.IGNORECASE,
            )
        )
        panel = (
            tag == "div"
            and self.stack[-1].attrs.get("class") == "yui-content"
            and re.fullmatch(r"wiki-tab-\d+-\d+", attrs.get("id") or "")
        )
        recognized_hidden = (
            tag == "div"
            and attrs.get("class") == "collapsible-block-unfolded"
            and self.stack[-1].attrs.get("class") == "collapsible-block"
        )
        exact_hidden_style = re.fullmatch(
            r"\s*display\s*:\s*none\s*;?\s*", style, re.IGNORECASE
        )
        if hidden and not (exact_hidden_style and (panel or recognized_hidden)):
            raise UnsupportedContent(f"hidden content in <{tag}>")
        if tag in {"td", "th"} and ("colspan" in attrs or "rowspan" in attrs):
            raise UnsupportedContent(f"table cell spans in <{tag}>")
        element = _Element(tag, attrs)
        self.stack[-1].children.append(element)
        if not void:
            self.stack.append(element)

    def handle_endtag(self, tag):
        if not self.stack or tag in _VOID:
            return
        if self.stack[-1].tag != tag:
            raise UnsupportedContent(f"unbalanced </{tag}> inside #page-content")
        self.stack.pop()

    def handle_data(self, data):
        if self.stack:
            self.stack[-1].children.append(data)

    def finish(self):
        if self.root is None or self.count != 1 or self.stack:
            raise UnsupportedContent("missing, duplicate, or unclosed #page-content")
        return self.root


def literal_text(value: str) -> str:
    """Keep source text literal rather than interpreting it as World Anvil BBCode."""
    if "[/noparse]" in value.lower():
        raise UnsupportedContent("literal text contains a noparse closing tag")
    if "[" in value or "]" in value:
        return f"[noparse]{value}[/noparse]"
    return value


def _significant_children(element: _Element) -> list[_Element]:
    children = [child for child in element.children if not isinstance(child, str)]
    if any(isinstance(child, str) and child.strip() for child in element.children):
        raise UnsupportedContent("unexpected text in collapsible structure")
    return children


def _collapsible_control(container: _Element) -> _Element:
    children = _significant_children(container)
    if len(children) != 1 or children[0].tag != "a":
        raise UnsupportedContent("invalid collapsible control")
    anchor = children[0]
    if anchor.attrs != {"class": "collapsible-block-link", "href": "javascript:;"}:
        raise UnsupportedContent("invalid collapsible control link")
    return anchor


def _render_collapsible(element, source_url, image_ref):
    children = _significant_children(element)
    if len(children) != 2:
        raise UnsupportedContent("invalid collapsible structure")
    folded, unfolded = children
    if (
        folded.attrs != {"class": "collapsible-block-folded"}
        or unfolded.attrs
        != {"class": "collapsible-block-unfolded", "style": "display:none"}
        or folded.tag != "div"
        or unfolded.tag != "div"
    ):
        raise UnsupportedContent("invalid collapsible panels")
    label_anchor = _collapsible_control(folded)
    if any(not isinstance(child, str) for child in label_anchor.children):
        raise UnsupportedContent("unsupported collapsible label")
    label = " ".join("".join(label_anchor.children).split())
    if not label or "[" in label or "]" in label or "[/noparse]" in label.lower():
        raise UnsupportedContent("unrepresentable collapsible label")
    expanded = _significant_children(unfolded)
    if (
        len(expanded) != 2
        or expanded[0].tag != "div"
        or expanded[0].attrs != {"class": "collapsible-block-unfolded-link"}
        or expanded[1].tag != "div"
        or expanded[1].attrs != {"class": "collapsible-block-content"}
    ):
        raise UnsupportedContent("invalid collapsible content")
    _collapsible_control(expanded[0])
    body = _render_children(expanded[1], source_url, image_ref).strip()
    return f"[spoiler={label}]{body}[/spoiler]"


def _url(value: str | None, source_url: str, kind: str) -> str:
    if not value:
        raise UnsupportedContent(f"missing {kind} URL")
    absolute = urljoin(source_url, value)
    parsed = urlsplit(absolute)
    if (
        parsed.scheme not in {"http", "https"}
        or not parsed.netloc
        or re.search(r"[\[\]|\s]", absolute)
    ):
        raise UnsupportedContent(f"unsafe or unrepresentable {kind} URL: {value}")
    return absolute


def _tab_content(script, navset, initializer, source_url, image_ref):
    expected_src = r"https://d3g0gp89917ko0\.cloudfront\.net/v--[\w-]+/common--javascript/yahooui/tabview-min\.js"
    tab_id = navset.attrs.get("id") or ""
    if (
        script.tag != "script"
        or not re.fullmatch(expected_src, script.attrs.get("src") or "")
        or navset.tag != "div"
        or navset.attrs.get("class") != "yui-navset"
        or not re.fullmatch(r"wiki-tabview-[0-9a-f]+", tab_id)
        or initializer.tag != "script"
        or initializer.attrs.get("src")
    ):
        raise UnsupportedContent("unrecognized YUI tabview")
    init = "".join(initializer.children)
    pattern = (
        r"\s*//<!\[CDATA\[\s*OZONE\.dom\.onDomReady\(function\(\)\s*\{\s*"
        r"var tabView[0-9a-f]+ = new YAHOO\.widget\.TabView\(\s*[\'\"]"
        + re.escape(tab_id)
        + r"[\'\"]\s*\);\s*\},\s*[\'\"]dummy-ondomready-block[\'\"]\);\s*//\]\]>\s*"
    )
    if not re.fullmatch(pattern, init):
        raise UnsupportedContent("unrecognized YUI tab initializer")
    children = [
        child
        for child in navset.children
        if not isinstance(child, str) or child.strip()
    ]
    if len(children) != 2:
        raise UnsupportedContent("unexpected YUI tab structure")
    navigation, content = children
    if (
        navigation.tag != "ul"
        or navigation.attrs.get("class") != "yui-nav"
        or content.tag != "div"
        or content.attrs.get("class") != "yui-content"
    ):
        raise UnsupportedContent("unexpected YUI tab structure")
    labels = [
        child
        for child in navigation.children
        if not isinstance(child, str) or child.strip()
    ]
    panels = [
        child
        for child in content.children
        if not isinstance(child, str) or child.strip()
    ]
    if not labels or len(labels) != len(panels):
        raise UnsupportedContent("YUI tab labels and panels differ")
    sections = []
    for index, (item, panel) in enumerate(zip(labels, panels)):
        label_children = [
            child
            for child in item.children
            if not isinstance(child, str) or child.strip()
        ]
        if item.tag != "li" or len(label_children) != 1 or label_children[0].tag != "a":
            raise UnsupportedContent("unsupported YUI tab label")
        anchor = label_children[0]
        emphasis = [
            child
            for child in anchor.children
            if not isinstance(child, str) or child.strip()
        ]
        if (
            anchor.attrs.get("href") != "javascript:;"
            or len(emphasis) != 1
            or emphasis[0].tag != "em"
        ):
            raise UnsupportedContent("unsupported YUI tab label")
        label = _render_children(
            emphasis[0], source_url, image_ref, block=False
        ).strip()
        if (
            not label
            or panel.tag != "div"
            or panel.attrs.get("id") != f"wiki-tab-0-{index}"
        ):
            raise UnsupportedContent("unmatched YUI tab panel")
        body = _render_children(panel, source_url, image_ref).strip()
        sections.append(f"[h2]{label}[/h2]\n{body}")
    return "\n".join(sections)


def _render_children(element, source_url, image_ref, *, block=True):
    parts: list[str] = []
    index = 0
    while index < len(element.children):
        child = element.children[index]
        index += 1
        if isinstance(child, _Element) and child.tag == "script":
            remaining = [
                item
                for item in element.children[index:]
                if not isinstance(item, str) or item.strip()
            ]
            if len(remaining) < 2 or not all(
                isinstance(item, _Element) for item in remaining[:2]
            ):
                raise UnsupportedContent("unsupported script in #page-content")
            rendered = _tab_content(
                child, remaining[0], remaining[1], source_url, image_ref
            )
            while (
                index < len(element.children)
                and element.children[index] is not remaining[1]
            ):
                index += 1
            index += 1
            if parts and parts[-1] and not parts[-1].endswith("\n"):
                parts.append("\n")
            parts.extend((rendered, "\n"))
            continue
        if isinstance(child, str):
            text = child if element.tag == "pre" else re.sub(r"\s+", " ", child)
            text = literal_text(text)
            if (
                not text.strip()
                and block
                and any(
                    isinstance(item, _Element) and item.tag in _BLOCK
                    for item in element.children
                )
            ):
                continue
            parts.append(text)
            continue
        rendered = _render(child, source_url, image_ref)
        if (
            block
            and child.tag in _BLOCK
            and parts
            and parts[-1]
            and not parts[-1].endswith("\n")
        ):
            parts.append("\n")
        parts.append(rendered)
        if block and child.tag in _BLOCK:
            parts.append("\n")
    return "".join(parts).strip("\n")


def _render(element, source_url, image_ref):
    tag = element.tag
    if tag == "script":
        raise UnsupportedContent("unsupported script in #page-content")
    if element.attrs.get("class") in {"yui-navset", "yui-nav", "yui-content"}:
        raise UnsupportedContent("unmatched YUI tab structure")
    if element.attrs.get("class") == "collapsible-block":
        return _render_collapsible(element, source_url, image_ref)
    if element.attrs.get("class") in {
        "collapsible-block-folded",
        "collapsible-block-unfolded",
        "collapsible-block-unfolded-link",
        "collapsible-block-content",
    }:
        raise UnsupportedContent("unmatched collapsible structure")
    if tag == "iframe":
        src = element.attrs.get("src") or ""
        title = element.attrs.get("title") or ""
        if (
            not set(element.attrs) <= _IFRAME_ATTRS
            or not _YOUTUBE_EMBED.fullmatch(src)
            or not title.strip()
            or any(
                not isinstance(child, str) or child.strip()
                for child in element.children
            )
        ):
            raise UnsupportedContent("unsupported iframe")
        return f"[url:{src}]{literal_text(title)}[/url]"
    if tag == "br":
        return "[br]"
    if tag == "hr":
        return "[hr]"
    if tag == "img":
        url = _url(element.attrs.get("src"), source_url, "image")
        reference = image_ref(url) if image_ref is not None else None
        if reference is not None:
            if type(reference) is not int or reference <= 0:
                raise UnsupportedContent(f"invalid image ID for {url}")
        else:
            reference = url
        return f"[img:{reference}|none]"
    if tag == "a":
        text = _render_children(element, source_url, image_ref, block=False)
        if element.attrs.get("href") == "javascript:;":
            return text
        url = _url(element.attrs.get("href"), source_url, "link")
        return f"[url:{url}]{text}[/url]"
    if tag in {"tbody", "thead", "tfoot", "table", "tr", "td", "th", "ul", "ol", "li"}:
        inner = _render_children(element, source_url, image_ref, block=False).strip()
    else:
        inner = _render_children(
            element, source_url, image_ref, block=tag in {"div", "blockquote"}
        )
        if tag in _BLOCK and tag != "pre":
            inner = inner.strip()
    if tag in {"div", "span", "tbody", "thead", "tfoot"}:
        return inner
    name = {"blockquote": "quote", "pre": "code", **_MARKUP}.get(tag, tag)
    return f"[{name}]{inner}[/{name}]"


def convert_html(
    html: str,
    source_url: str,
    image_ref: Callable[[str], int | None] | None = None,
) -> str:
    """Convert exactly one rendered div#page-content; never fetch or mutate anything."""
    parser = _PageParser()
    parser.feed(html)
    parser.close()
    root = parser.finish()
    return _render_children(root, source_url, image_ref).strip()
