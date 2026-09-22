"""Generate protected POC CSS from the acquired admin:css and admin:font sources."""

import argparse
import os
from pathlib import Path
import re

from .listing_export import _prepare_path

FONT_IMPORT = "@import url(/admin:font/code/1);"
SOURCE_FILES = "http://cobalt-company.wdfiles.com/local--files/"


def extract_css(source):
    blocks = re.findall(r'\[\[code type="css"\]\](.*?)\[\[/code\]\]', source, re.S)
    if len(blocks) != 1:
        raise ValueError("expected exactly one acquired CSS code block")
    return blocks[0].strip()


def render_theme(theme_source, font_source):
    theme = extract_css(theme_source)
    font = extract_css(font_source)
    if theme.count(FONT_IMPORT) != 1:
        raise ValueError("source theme must import the acquired font exactly once")
    return theme.replace(FONT_IMPORT, font).replace(SOURCE_FILES, "/-/file/") + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--theme-source", type=Path, required=True)
    parser.add_argument("--font-source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        css = render_theme(args.theme_source.read_text(), args.font_source.read_text())
        _prepare_path(args.output)
        fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "w") as stream:
            stream.write(css)
    except (ValueError, OSError) as error:
        parser.exit(1, f"Theme generation failed: {error}\n")
    print("Generated protected stylesheet; original sources unchanged.")


if __name__ == "__main__":
    main()
