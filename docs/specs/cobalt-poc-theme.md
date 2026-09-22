# Cobalt POC theme

Restore styling from acquired `admin:css` and `admin:font`; do not approximate the design with another site's theme.

- Generate one owner-only CSS file without changing archived page sources.
- Embed the acquired font CSS and redirect source attachment URLs to same-origin imported files.
- Serve `/-/cobalt-theme.css` behind the same authentication gate as pages and attachments.
- Select the source theme only for `cobalt-company`; preserve other sites' existing theme.
- Verify generated CSS, authentication, resource loading and browser appearance. Import can continue independently.

## Runtime

Run `python -m tools.cobalt_migration.poc_theme --theme-source <protected-admin-css-source> --font-source <protected-admin-font-source> --output <new-private-file>`. Provision output as `/var/lib/cobalt-wiki/poc-theme.css`, owner `cobalt-wiki`, mode `0600`, before deploying theme selection.

Tests: `tests/cobalt_migration/test_poc_theme.py`. Full source layout/includes remain separate from stylesheet restoration; see [POC import](cobalt-poc-import.md).
