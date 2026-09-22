# Cobalt backup inventory

`tools/cobalt_migration/` inventories native Wikidot gzip-compressed TAR backups without extracting files. The standard-library-only Python 3.11+ CLI produces a content-free manifest for later migration reconciliation; it does not establish page identities or import data.

```text
python -m tools.cobalt_migration ARCHIVE.tar.gz --output PROTECTED_DIRECTORY/manifest.json
```

The caller provisions the protected output directory. Manifests contain potentially private archive names and must remain outside git.

## What it must do

- [x] Require an explicit archive and output path; never replace the input archive.
- [x] Record the compressed archive SHA-256 and each regular file's exact archive path, role, byte size, and SHA-256; sort records by path for deterministic JSON.
- [x] Classify regular files under `source/` as `page_source` and under `files/` as `attachment`; report counts and uncompressed byte totals by role. Ignore safe directory entries in those totals.
- [x] Validate all page-source bytes as strict UTF-8, including chunk boundaries and incomplete final sequences. Hash original bytes without normalization; never include source contents or field values in the manifest or decoding errors.
- [x] Reject malformed/truncated gzip or TAR data, corrupt gzip checksums, invalid later headers, missing TAR terminators, unexpected trailing data, unsupported regular-file roots, duplicate file paths, unsafe paths, links, and special files.
- [x] Publish complete JSON through atomic replacement with mode `0600`; retain prior output on failure and remove temporary output files.

The JSON object has exactly these top-level keys:

- `archive_sha256`: SHA-256 of the compressed input bytes.
- `summary`: `page_source` and `attachment`, each containing `count` and `total_bytes`.
- `files`: path-sorted objects with `path`, `role`, `size`, and `sha256`.

Underscores, Unicode names, and template names are retained. Native archives replace colons with underscores; inventory must not infer the original slug, title, tags, author, or category metadata from those names. Safe directories need not have a regular-file role. Source records are classified by root, not filename extension.

Success exits `0` without printing the manifest. Inventory/write failures exit `1` with a contextual error; argument errors use argparse's exit `2`.

## How it works

- [Inventory module](../../tools/cobalt_migration/archive.py): streamed validation, hashing, and atomic JSON output.

## Implementation inventory

- `tools/cobalt_migration/archive.py`: inventory API, manifest writer, and CLI arguments/errors.
- `tools/cobalt_migration/__init__.py`: public `ArchiveError`, `inventory_archive`, and `write_manifest` exports.
- `tools/cobalt_migration/__main__.py`: module entry point.

## Tests asserting this spec

`tests/cobalt_migration/test_archive.py` uses synthetic TAR/gzip fixtures and real CLI subprocesses. It checks exact manifest values, Unicode and binary data, counts, hashes, deterministic output, atomic replacement observed by an existing reader, file permissions, failure cleanup, and each corruption/path/type boundary.

```text
python -B -m unittest discover -s tests/cobalt_migration -p test_archive.py -v
```

## Known gaps (current cycle)

- [ ] Real-backup inventory and independent integration verification belong to the migration owner; synthetic tests do not prove source/target parity.

## Out of scope

Archive extraction, slug mapping, YAML parsing, metadata acquisition, importing, source-site changes, and deployment belong to subsequent migration work. No production data or private fixture is stored in this repository.
