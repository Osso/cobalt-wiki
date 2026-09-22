"""Run with python -m tools.cobalt_migration ARCHIVE --output MANIFEST."""

from .archive import main

if __name__ == "__main__":
    raise SystemExit(main())
