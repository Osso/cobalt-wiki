# Cobalt native backup inventory

This operator step inventories a native Wikidot `.tar.gz` backup before migration. It does not extract content, map pages, or write to a target wiki. Contract: [backup inventory spec](../../specs/cobalt-backup-inventory.md).

## Run

Keep both archive and manifest outside the repository. The standard protected location is `/home/osso/.local/share/cobalt-wiki/source`.

```text
mkdir -p /home/osso/.local/share/cobalt-wiki/source
chmod 700 /home/osso/.local/share/cobalt-wiki /home/osso/.local/share/cobalt-wiki/source
python -B -m tools.cobalt_migration /absolute/path/to/backup.tar.gz \
  --output /home/osso/.local/share/cobalt-wiki/source/backup-manifest.json
```

The output parent must already exist and be owner-only. The CLI does not create or validate it; it atomically writes the manifest itself with mode `0600`. Never place an archive, manifest, signed download URL, token, or source profile content in git.

## Manifest

The JSON object contains exactly:

- `archive_sha256`: SHA-256 of compressed backup bytes.
- `summary.page_source` and `summary.attachment`: each has `count` and `total_bytes`.
- `files`: path-sorted `{path, role, size, sha256}` records.

`page_source` means a regular file below `source/`; `attachment` means one below `files/`. Hashes and sizes cover original uncompressed member bytes. Page-source bytes must be valid UTF-8, but their contents are not emitted.

## Archive keys and limits

Native paths are retained exactly. A source page is stored as `source/<native-key>.txt`; an attachment as `files/<native-key>/<filename>`. The native export turns Wikidot colons into underscores. These keys are not a reliable source of original slugs, titles, tags, authors, or category metadata, so this tool deliberately does not infer them.

This is inventory only. No source metadata/API export, YAML form transformation, permissions mapping, import, theme replication, DNS, service deployment, or data is present at `cobalt-company.sakuin.org` yet.
