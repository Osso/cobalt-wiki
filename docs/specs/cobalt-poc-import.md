# Cobalt POC import

`tools/cobalt_migration/poc_import.py` imports native page sources and attachments through Deepwell RPCs. The approved local missing-only slice creates only identities absent from a fresh, complete target inventory; it neither changes the protected 6,092-page/1,471-file plan nor edits, adopts, restores, or byte-compares existing identities. This is a contract, not a completion claim. See [backup inventory](cobalt-backup-inventory.md) and [metadata acquisition](cobalt-metadata-export.md).

## What it must do

- [x] Validate all archive members and a complete canonical listing before writing a protected immutable plan; reject duplicate/colliding identities, missing/extra source pages, and orphan attachment owners.
- [x] Retain exact canonical page names, including multiple colons, source bytes and attachment bytes; verify target readback with SHA-256 and size.
- [x] Resume after a committed page creation whose response was lost without creating duplicate pages.
- [x] Refuse existing records not marked with this plan and technical principal; refuse modified imported content rather than overwrite it.
- [x] Preserve acquired title/tags and supplemental source revision/time metadata. Missing metadata remains explicitly unacquired; canonical fullname is the POC display label, not a claimed original title.
- [x] Write plans as owner-only files; reject changed archives before target calls.
- [x] Reconcile the full protected 6,092-source/1,471-attachment archive against a real provisioned Deepwell instance.
- [x] Prove actual `page_import` exact-name readback, attachment upload/readback, technical-principal authorization and metadata retention in the native runtime. Source ACL parity remains separate.
- [ ] For the approved **local missing-only** slice, retain the immutable 6,092-page/1,471-file plan unchanged and select only plan entries absent from a fresh, complete inventory bound to that plan, target site, and RPC endpoint.
- [ ] Treat every current, historical, and deleted page/file identity as present for selection. Do not compare its bytes, tags, metadata, attribution, or importer ownership, and do not edit, replace, adopt, restore, or otherwise mutate it.
- [ ] Reject inventories with orphan audit page IDs. Skip an attachment whose current owner cannot be determined uniquely; do not infer an owner from historical identities.
- [ ] Create only the selected bounded page/file set. Create every selected page with its native `page_import` tags in the same atomic creation operation; do not issue a follow-up edit for tags.
- [ ] Before every mutation run, obtain a new complete site inventory and validate it against the immutable plan and actual RPC endpoint. On any mutation failure, stop; restart only from a newly acquired and validated inventory, never by blindly retrying a mutation.

## How it works

- [Native runtime](cobalt-native-runtime.md)
- [Canonical listing export](cobalt-listing-export.md)

## Implementation inventory

- `tools/cobalt_migration/poc_import.py`: existing plan/apply CLI, trusted loopback JSON-RPC, and presigned PUT adapter; target module for the bounded local missing-only runner.
- `tools/cobalt_migration/poc_missing_inventory.py`: pure validated selector of absent plan entries; preserves every existing/current/historical/deleted identity and skips ambiguous attachment owners.
- Deepwell `page_import`: required native atomic exact-name page/first-revision/tag creation with technical attribution; no fallback to normalizing `page_create` or a tag-edit follow-up.
- Deepwell `page_get`, `page_edit`, `blob_upload`, `file_create`, `file_get`: reconciliation, tags and attachment persistence/readback.

## Tests asserting this spec

- `tests/cobalt_migration/test_poc_import.py`: synthetic archives and a persistent in-memory RPC datastore; exact multi-colon identity/content, collision refusal, restart after lost response without duplicate pages or attachments, non-import conflict, changed archive/content.

## Known gaps (current cycle)

- [x] Full archive apply completed with 6,092 pages and 1,471 attachments (verified: 2026-09-22). SQL reconciliation independently matches every page's name/title/tags/source size/SHA-256 and migration attribution. Attachment ownership/name/size inventory matches; apply verified all attachment bytes. See [current proof](../wiki/systems/cobalt-replica-status.md).
- [x] Unmatched `))` is accepted through corrected FTML token dispatch. The exact archive fixture, native DB/rendering regression and complete 6,092-source parser corpus pass. Ordinary `page_create` normalization remains unchanged; logging suppression and blind mutation retries were reverted.
- [ ] Provisioning must supply an existing positive-ID target site and a dedicated technical import principal with an authenticated session authorized to edit/import that site. The principal may be the seeded administrator (ID −1) or a positive user ID; the session identity must match the immutable plan. Runtime must allow the source archive's largest pages/attachments.
- [ ] Deepwell and presigned S3 endpoints must be loopback IPv4/IPv6 literals (run on target or forward both ports). Session file and plan must be `0600` in an owner-only directory.
- [ ] Host owner must protect every POC route before import. No source ACL parity is claimed.
- [ ] Run the local missing-only slice only against a local target with one exclusive writer. Concurrent human edits, another importer, production targets, service administration, and database operations are excluded.
- [ ] Before each bounded run, use a fresh complete inventory from the exact target site and endpoint. Any mutation failure aborts the run; a later run starts from a new inventory and never blindly retries the failed mutation.
- [ ] A failed upload may leave an unfinalized pending blob; this importer never deletes target objects.

## Out of scope

- Site/user provisioning, authentication gate, deployment, source author identity mapping, full revision history, and rendering parity.
- Target revision creation dates/author identify the technical import event, not source creation. Original acquired revision number and last-modified timestamp remain in the protected plan, not forged target history.
- Production/site-service/database operations, delegation, updates after human edits, automatic adoption of records created by another plan, restoration of deleted identities, and reconciliation of existing bytes or ownership.
