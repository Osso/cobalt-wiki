# Cobalt POC import

`tools/cobalt_migration/poc_import.py` imports latest native page sources and attachments through existing Deepwell RPCs. It does not create sites/users or implement database compression, blob addressing, or revision storage. See [backup inventory](cobalt-backup-inventory.md) and [metadata acquisition](cobalt-metadata-export.md).

## What it must do

- [x] Validate all archive members and a complete canonical listing before writing a protected immutable plan; reject duplicate/colliding identities, missing/extra source pages, and orphan attachment owners.
- [x] Retain exact source bytes and attachment bytes; verify target readback with SHA-256 and size.
- [x] Resume after a committed page creation whose response was lost without creating duplicate pages.
- [x] Refuse existing records not marked with this plan and technical principal; refuse modified imported content rather than overwrite it.
- [x] Preserve acquired title/tags and supplemental source revision/time metadata. Missing metadata remains explicitly unacquired; canonical fullname is the POC display label, not a claimed original title.
- [x] Write plans as owner-only files; reject changed archives before target calls.
- [ ] Reconcile the full protected 6,092-source/1,471-attachment archive against a real provisioned Deepwell instance.
- [ ] Prove actual RPC upload/readback, authorization, parser-limit and metadata-normalization behavior in the native runtime.

## How it works

- [Native runtime](cobalt-native-runtime.md)
- [Canonical listing export](cobalt-listing-export.md)

## Implementation inventory

- `tools/cobalt_migration/poc_import.py`: plan/apply CLI, exclusive import reconciliation, trusted loopback JSON-RPC and presigned PUT adapter.
- Existing Deepwell `page_create`, `page_get`, `page_edit`, `blob_upload`, `file_create`, `file_get`: target persistence and readback.

## Tests asserting this spec

- `tests/cobalt_migration/test_poc_import.py`: synthetic archives and a persistent in-memory RPC datastore; exact content, collision refusal, restart after lost response, non-import conflict, changed archive/content.

## Known gaps (current cycle)

- [ ] Real endpoint acceptance remains pending; mocked datastore proof is not database/network integration proof.
- [ ] Provisioning must supply an existing target site and dedicated technical import user with an authenticated session authorized to edit/import that site. Runtime must allow the source archive's largest pages/attachments.
- [ ] Deepwell and presigned S3 endpoints must be loopback IPv4/IPv6 literals (run on target or forward both ports). Session file and plan must be `0600` in an owner-only directory.
- [ ] Host owner must protect every POC route before import. No source ACL parity is claimed.
- [ ] Run import exclusively: concurrent human edits or another importer are unsupported and conflicting readback stops execution.
- [ ] A failed upload may leave an unfinalized pending blob; this importer never deletes target objects or blindly retries create/edit RPCs. A restart reconciles committed objects.

## Out of scope

- Site/user provisioning, authentication gate, deployment, source author identity mapping, full revision history, and rendering parity.
- Target revision creation dates/author identify the technical import event, not source creation. Original acquired revision number and last-modified timestamp remain in the protected plan, not forged target history.
- Updates after human edits or automatic adoption of records created by another plan.
