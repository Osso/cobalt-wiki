"""Decode Wikidot's formatted historical source display, not original bytes."""

from html import unescape
from html.entities import html5
from html.parser import HTMLParser


class HistorySourceError(ValueError):
    """The source display cannot be decoded without guessing its structure."""


class _SourceHTML(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=False)
        self.found = 0
        self.inside = False
        self.closed = False
        self.pending_break = False
        self.text = []

    def handle_starttag(self, tag, attrs):
        if self.inside:
            if tag != "br" or attrs or self.pending_break:
                raise HistorySourceError("unexpected markup inside historical source")
            self.pending_break = True
            return
        if tag == "div" and "page-source" in dict(attrs).get("class", "").split():
            if self.found:
                raise HistorySourceError("multiple historical source containers")
            self.found = 1
            self.inside = True

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)

    def handle_endtag(self, tag):
        if not self.inside:
            return
        if tag != "div" or self.pending_break:
            raise HistorySourceError("malformed historical source container")
        self.inside = False
        self.closed = True

    def handle_data(self, data):
        if not self.inside:
            if self.closed and data.endswith("<"):
                raise HistorySourceError("malformed trailing HTML")
            return
        if self.pending_break:
            if not data.startswith("\n"):
                raise HistorySourceError("line break is not followed by source newline")
            self.pending_break = False
        self.text.append(data)

    def handle_entityref(self, name):
        if not self.inside:
            return
        if self.pending_break or name + ";" not in html5:
            raise HistorySourceError("invalid source entity")
        # semipre changes indentation and runs of spaces to &nbsp;.
        self.text.append(" " if name == "nbsp" else unescape("&" + name + ";"))

    def handle_charref(self, name):
        if not self.inside:
            return
        try:
            value = int(name[1:], 16) if name.lower().startswith("x") else int(name)
        except ValueError:
            raise HistorySourceError("invalid source character reference") from None
        if self.pending_break or not 0 < value <= 0x10FFFF or 0xD800 <= value <= 0xDFFF:
            raise HistorySourceError("invalid source character reference")
        self.text.append(chr(value))


def decode_history_source(html):
    """Return displayed wikitext with its non-byte-exact provenance marker."""
    if not isinstance(html, str):
        raise HistorySourceError("historical source HTML must be text")
    parser = _SourceHTML()
    parser.feed(html)
    parser.close()
    if parser.found != 1 or not parser.closed or parser.pending_break:
        raise HistorySourceError("missing or incomplete historical source container")
    displayed = "".join(parser.text)
    if (
        not displayed.startswith("\n")
        or not displayed.endswith("\n")
        or len(displayed) < 2
    ):
        raise HistorySourceError("historical source wrapper newline missing")
    return {
        "wikitext": displayed[1:-1],
        "representation": "display-decoded-not-byte-exact",
    }
