# Cobalt history import

Cobalt history import must backfill authorized source revision history without presenting the latest-source migration as complete history. The current migration has 6,092 pages and 1,471 files. A bounded homepage pilot acquired all 240 listed revision records and three historical source-module representations; it did not establish byte-exact historical bodies or whole-site history counts. See the [current POC import contract](cobalt-poc-import.md), the [native revision import endpoint](../../deepwell/src/endpoints/import.rs), the [native import service](../../deepwell/src/services/import/service.rs), and the [stock WikiComma importer entry point](../../deepwell/importer/__main__.py).

## What it must do

### Authorized source acquisition

- [ ] Acquire only authorized, available source revision records: exact bodies, global IDs, per-page numbers, timestamps, author IDs, comments, and revision-specific metadata.
- [ ] Preserve exact source revision bodies and prove final-body hash equality before production import. Current source-module decoding is not a generic byte-exact recovery method.
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

- [x] Acquire all 240 revision-list records (numbers 0–239) for one page and inspect three historical source-module representations through the authenticated read-only source UI.
- [x] Confirm that the module returns HTML-wrapped source representations rather than an independently verified byte-identical archive representation.
- [ ] Complete a reversible local multi-revision backfill pilot before any production history write.
- [ ] Prove imported final-body equality by an unambiguous source hash; DOM-decoded body hashes alone are insufficient.
- [ ] Establish a source-backed byte-fidelity contract. The current-revision decoded calibration hash matches the archive after transport decoding, but sampled historical representations remain unproven; do not promise a generic lossless decoder.
- [ ] Prevent historical bulk work from causing an outdate or rerender flood; validate queue impact before production work.
- [ ] Require review before selecting an implementation, identifier-rekeying policy, deletion policy, or replacement/backfill policy.

## How it works

- [POC import](cobalt-poc-import.md)
- [Queue incident and recovery](../wiki/systems/cobalt-queue-recovery.md)
- [Stock WikiComma importer implementation](../../deepwell/importer/importer.py)
- [Stock WikiComma archive reader](../../deepwell/importer/site.py)
- [Native revision import endpoint](../../deepwell/src/endpoints/import.rs)
- [Native revision import service](../../deepwell/src/services/import/service.rs)

## Implementation inventory

- `deepwell/importer/site.py`: reads revision metadata and per-page `.7z` bodies keyed by revision number.
- `deepwell/importer/importer.py`: writes the stock SQLite/S3 target.
- `deepwell/importer/__main__.py`: importer command entrypoint; it does not crawl source history.
- `deepwell/src/endpoints/import.rs`: exposes `import_wikidot_page_revision`.
- `deepwell/src/services/import/service.rs`: native Wikidot revision import behavior; the endpoint expects a sequential history beginning at revision 0.
- `tools/cobalt_migration/poc_import.py`: current latest-source importer; it does not acquire or import historical revision records.
- `tools/cobalt_migration/page_history.py`: parses revision-list and historical-source module representations.
- `tools/cobalt_migration/history_export.py`: checkpoints one-page authenticated history acquisition.

## Tests asserting this spec

`tests/cobalt_migration/test_page_history.py` has seven focused parser/export tests pending independent verification. Read-only pilot artifacts, field-inventory proof, and decoding calibration remain protected under `/home/osso/.local/share/cobalt-wiki/source/history-pilot/`; they establish source availability and capture only, not import behavior or historical byte fidelity.

## Known gaps (current cycle)

- [x] Verify homepage revision-list pagination and IDs: 12 pages of 20 rows cover each revision number 0–239 exactly once; numeric author/date fields were captured for every listed row and three historical HTML bodies were captured. This establishes only that page's listed history.
- [ ] Acquire every required page's authorized revisions and establish complete counts/pagination. Current latest-revision positions total 45,367 across 6,090 accepted pages plus two unresolved pages; this is not a verified historical revision count.
- [ ] Review a backfill design for pages whose technical import already occupies revision 0; straight append of source revision 0 conflicts with the native sequential requirement.
- [ ] Build and prove a reversible local multi-revision backfill pilot, including unambiguous body equality and bounded queue impact.
- [ ] Establish source-backed creator and ACL dependencies before claiming permission parity.

## Out of scope

- Source cutover or deletion.
- API-key dependency.
- Engine change.
- Rendering work owned by the user.
- Choosing an implementation, identifier-rekeying policy, deletion policy, or replacement/backfill policy before review.
