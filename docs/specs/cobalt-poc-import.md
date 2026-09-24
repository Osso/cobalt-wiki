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
- [x] For the approved **local missing-only** slice, retain the immutable 6,092-page/1,471-file plan unchanged and select only plan entries absent from a fresh, complete inventory bound to that plan, target site, and RPC endpoint.
- [x] Treat every current, historical, and deleted page/file identity as present for selection. Do not compare its bytes, tags, metadata, attribution, or importer ownership, and do not edit, replace, adopt, restore, or otherwise mutate it.
- [x] Reject inventories with orphan audit page IDs. Skip an attachment whose current owner cannot be determined uniquely; do not infer an owner from historical identities.
- [x] Create only the selected bounded page/file set. Create every selected page with its native `page_import` tags in the same atomic creation operation; do not issue a follow-up edit for tags.
- [x] Before every mutation run, obtain a new complete site inventory and validate it against the immutable plan and actual RPC endpoint. On any mutation failure, stop; restart only from a newly acquired and validated inventory, never by blindly retrying a mutation.

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

## Completed local missing-only import

Verified: 2026-09-24. The immutable plan remained 6,092 pages and 1,471 files. The completed local run added 2,108 absent pages and 966 absent files; each added raw page/file hash read back unchanged. The target now has 6,101 active pages (6,092 planned plus nine pre-existing fixtures) and 1,471 files, with no plan entries remaining.

- Original page/file rows, file revisions, and nine imported-history rows remained unchanged.
- A fresh inventory after injected lost native page-creation and file-creation responses excluded the already committed identity, proving restart does not duplicate either kind.
- Search is idle with 6,101 documents and zero pending updates; queue depth is the four baseline periodic jobs. GC containers remain stopped. Protected HTTP smoke proof records `/new-writing` returning `200` and native `page_search` returning the exact `new-writing` slug among 20 results.
- Only existing SQL migration `20260923000005` ran locally, creating the missing `wikidot_site_change` table required by backend rendering. It did not acquire site-history rows.
- The earlier `da8a05a` gate executed 44 test cases, but that count includes duplicated global discovery of the imported `ImportTests` fixture and is not a unique-test count. Test-only `f8a4d71` removes that discovery duplication through a module alias; importer code remains `af26ce5`. At `af26ce5`, 12 targeted type-error tests plus Ruff and formatting checks passed. Aggregate gate: `/tmp/cobalt-company-wiki-missing-only-gate-af26ce5.log`. At `f8a4d71`, all five missing-only application tests and scoped Ruff/format checks passed. Discovery confirms 32 distinct tests across the three modules (15 original, 12 inventory, five application); no redundant combined rerun. Independent artifact audit: `/tmp/claude/cobalt-missing-final-audit.log`.

### Preservation limitation

Do **not** claim every original native revision is byte-identical. The initial native-revision fingerprint intentionally excluded compiled outputs but mistakenly included `updated_at`, which native rerendering mutates. Of 61 early changed fingerprints, 18 reconstruct exactly when `updated_at` is normalized to null; the other 43 original full hashes cannot be reconstructed. Original page/file rows remain identical, and normalized snapshots of source content and metadata from the 4,055-page checkpoint onward match. Later original timestamp changes are acceptable only while that final normalized baseline remains stable.

This completes the bounded local missing-page/file operation, not blanket full-replica readiness. Source ACL parity, author/history parity, rendering/visual parity, deployment, and other status boundaries remain separate. See [current proof](../wiki/systems/cobalt-replica-status.md).

## Known gaps (current cycle)

- [x] Unmatched `))` is accepted through corrected FTML token dispatch. The exact archive fixture, native DB/rendering regression and complete 6,092-source parser corpus pass. Ordinary `page_create` normalization remains unchanged; logging suppression and blind mutation retries were reverted.
- [ ] Provisioning must supply an existing positive-ID target site and a dedicated technical import principal with an authenticated session authorized to edit/import that site. The principal may be the seeded administrator (ID −1) or a positive user ID; the session identity must match the immutable plan. Runtime must allow the source archive's largest pages/attachments.
- [ ] Deepwell and presigned S3 endpoints must be loopback IPv4/IPv6 literals (run on target or forward both ports). Session file and plan must be `0600` in an owner-only directory.
- [ ] Host owner must protect every POC route before import. No source ACL parity is claimed.
- [ ] Run the local missing-only slice only against a local target with one exclusive writer. Concurrent human edits, another importer, production targets, service administration, and database operations are excluded.
- [x] The completed local run used fresh inventories from the exact target site and endpoint; injected lost responses were resolved only by a later fresh inventory, never by blindly retrying a mutation.
- [ ] A failed upload may leave an unfinalized pending blob; this importer never deletes target objects.

## Out of scope

- Site/user provisioning, authentication gate, deployment, source author identity mapping, full revision history, and rendering parity.
- Target revision creation dates/author identify the technical import event, not source creation. Original acquired revision number and last-modified timestamp remain in the protected plan, not forged target history.
- Production/site-service/database operations, delegation, updates after human edits, automatic adoption of records created by another plan, restoration of deleted identities, and reconciliation of existing bytes or ownership.
