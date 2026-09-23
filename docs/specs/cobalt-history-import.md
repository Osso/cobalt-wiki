# Cobalt history import

Cobalt history import must backfill authorized source revision history without presenting the latest-source migration as complete history. The current migration has 6,092 pages and 1,471 files, but the historical revision count and records have not been acquired. See the [current POC import contract](cobalt-poc-import.md), the [native revision import endpoint](../../deepwell/src/endpoints/import.rs), the [native import service](../../deepwell/src/services/import/service.rs), and the [stock WikiComma importer reference](/home/osso/Repos/wikijump/deepwell/importer/README.md).

## What it must do

### Authorized source acquisition

- [ ] Acquire only authorized, available source revision records: exact bodies, global IDs, per-page numbers, timestamps, author IDs, comments, and revision-specific metadata.
- [ ] Preserve exact source revision bodies and prove final-body hash equality before production import.
- [ ] Record unavailable or inaccessible revisions as explicit gaps; do not infer, synthesize, or silently omit them.
- [ ] Keep source creator and ACL dependencies explicitly unknown until authorized source evidence establishes them.
- [ ] Avoid an API-key dependency for acquisition or import.

### Backfill integrity

- [ ] Preserve the current imported page and latest source while a reviewed backfill runs.
- [ ] Preserve source revision order, source identifiers, numbers, timestamps, author identities, comments, and revision-specific metadata when acquired.
- [ ] Never fabricate revision records, bodies, author identities, or metadata.
- [ ] Distinguish the technical migration record already present on POC pages from source history; do not represent it as a source revision.
- [ ] Reconcile every imported record and explicitly report retained gaps after backfill.

### Proof and operational safety

- [ ] Complete a local authorized multi-revision-page pilot before any production history work.
- [ ] Prove the pilot's final revision body matches the authorized source body by hash.
- [ ] Prevent historical bulk work from causing an outdate or rerender flood; validate queue impact before production work.
- [ ] Require review before selecting an implementation, identifier-rekeying policy, deletion policy, or replacement/backfill policy.

## How it works

- [POC import](cobalt-poc-import.md)
- [Queue incident and recovery](../wiki/systems/cobalt-queue-recovery.md)
- [Stock WikiComma importer implementation](/home/osso/Repos/wikijump/deepwell/importer/importer.py)
- [Stock WikiComma archive reader](/home/osso/Repos/wikijump/deepwell/importer/__main__.py)
- [Native revision import endpoint](../../deepwell/src/endpoints/import.rs)
- [Native revision import service](../../deepwell/src/services/import/service.rs)

## Implementation inventory

- `/home/osso/Repos/wikijump/deepwell/importer/importer.py`: stock WikiComma reference importer consumes `metadata["revisions"]` and per-page `.7z` revision bodies keyed by revision number, writing its own SQLite/S3 target.
- `/home/osso/Repos/wikijump/deepwell/importer/__main__.py`: stock importer command entrypoint; it does not crawl source history.
- `deepwell/src/endpoints/import.rs`: exposes `import_wikidot_page_revision`.
- `deepwell/src/services/import/service.rs`: native Wikidot revision import behavior; the endpoint expects a sequential history beginning at revision 0.
- `tools/cobalt_migration/poc_import.py`: current latest-source importer; it does not acquire or import historical revision records.

## Tests asserting this spec

None. No authorized source history, revision count, pilot fixture, or backing history-import test has been acquired.

## Known gaps (current cycle)

- [ ] Acquire authorized source revision records and determine the historical revision count.
- [ ] Review a backfill design for pages whose technical import already occupies revision 0; straight append of source revision 0 conflicts with the native sequential requirement.
- [ ] Build and prove one local multi-revision history pilot, including final-body hash equality and bounded queue impact.
- [ ] Establish source-backed creator and ACL dependencies before claiming permission parity.

## Out of scope

- Source cutover or deletion.
- API-key dependency.
- Engine change.
- Rendering work owned by the user.
- Choosing an implementation, identifier-rekeying policy, deletion policy, or replacement/backfill policy before review.
