# Cobalt history import

Cobalt history import must backfill authorized source revision history without presenting the latest-source migration as complete history. The current migration has 6,092 pages and 1,471 files. The user approved importing historical text from Wikidot's display representation even where tabs may become spaces; raw responses and that limit must remain recorded. The bounded homepage pilot acquired 240 listed revisions and all 240 source-module representations; it did not establish byte-exact historical bodies or whole-site history counts. See the [current POC import contract](cobalt-poc-import.md), the [native revision import endpoint](../../deepwell/src/endpoints/import.rs), the [native import service](../../deepwell/src/services/import/service.rs), and the [stock WikiComma importer entry point](../../deepwell/importer/__main__.py).

## What it must do

### Authorized source acquisition

- [ ] Acquire only authorized, available source revision records: source IDs, per-page numbers, timestamps, author IDs, comments, revision-specific metadata, raw source-module responses, and their display-decoded bodies.
- [ ] Preserve the raw response and mark each decoded historical body as display-decoded, not byte-exact. The approved tab-to-space limit does not apply to the current archived source, which remains byte-exact.
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

- [x] Acquire all 240 revision-list records (numbers 0–239) and all 240 historical source-module responses for one page through the authenticated read-only source UI.
- [x] Decode all 240 captured source-module representations with explicit display-decoded provenance; the current revision decodes to the archived source hash.
- [x] Confirm that the module returns HTML-wrapped source representations rather than an independently verified byte-identical archive representation.
- [ ] Complete a reversible local multi-revision backfill pilot before any production history write.
- [ ] Prove imported current-body equality by the existing archived source hash.
- [ ] Keep historical display-decoding provenance and raw responses through import. Do not promise byte-exact recovery for old bodies.
- [ ] Prevent historical bulk work from causing an outdate or rerender flood; validate queue impact before production work.
- [ ] Require review before selecting an implementation, identifier-rekeying policy, deletion policy, or replacement/backfill policy.

## How it works

- [POC import](cobalt-poc-import.md)
- [Queue incident and recovery](../wiki/systems/cobalt-queue-recovery.md)
- [Stock WikiComma importer implementation](../../deepwell/importer/importer.py)
- [Stock WikiComma archive reader](../../deepwell/importer/site.py)
- [Native revision import endpoint](../../deepwell/src/endpoints/import.rs)
- [Native revision import service](../../deepwell/src/services/import/service.rs)

## Local backend history storage

Imported source history uses `imported_page_revision`, separate from native editable revisions. Unknown historical title/slug/tags remain null; source attachment events retain their original flags. Import does not change the current revision or render pages. Read endpoints enforce current page visibility; imported entries are not native rollback targets.

- [x] Import three source records idempotently without changing current source, compiled output, or native revision count.
- [x] Read history across two cursor pages and retrieve an old body with its source author ID and representation marker.
- [ ] Verify conflict rollback, stale-current-revision rejection, visibility denial, and zero queue impact.
- [ ] Run the acquired 240-revision pilot through this storage path and expose it in the history UI with the rendering owner.

`import_wikidot_history` accepts a guarded current revision ID and source records. `page_imported_history` lists at most 100 records, descending by source revision number; `before_revision` is exclusive. `page_imported_revision` retrieves a source body. These loopback backend APIs are not yet deployed.

## Implementation inventory

- `deepwell/importer/site.py`: reads revision metadata and per-page `.7z` bodies keyed by revision number.
- `deepwell/importer/importer.py`: writes the stock SQLite/S3 target.
- `deepwell/importer/__main__.py`: importer command entrypoint; it does not crawl source history.
- `deepwell/src/endpoints/import.rs`: exposes `import_wikidot_page_revision`.
- `deepwell/src/services/import/service.rs`: native Wikidot revision import behavior; the endpoint expects a sequential history beginning at revision 0.
- `tools/cobalt_migration/poc_import.py`: current latest-source importer; it does not acquire or import historical revision records.
- `tools/cobalt_migration/page_history.py`: parses revision-list metadata and gaps.
- `tools/cobalt_migration/history_source.py`: decodes the source display with explicit non-byte-exact provenance.
- `tools/cobalt_migration/poc_import.py`: remains latest-source only; no history target write exists yet.

- `deepwell/src/services/import/history.rs` and `history_structs.rs`: guarded import and permission-checked reads.
- `deepwell/migrations/20260923000000_imported_page_revision.sql`: separate source-history storage.

## Tests asserting this spec

- `deepwell/tests/imported_history.rs`: native import/idempotence/current-page preservation and real two-page history reads.

`tests/cobalt_migration/test_page_history.py` has nine independently verified parser tests. `tests/cobalt_migration/test_history_source.py` has five decoder tests; all 240 protected pilot responses decode, and revision 239 matches the existing archived homepage hash. Protected artifacts under `/home/osso/.local/share/cobalt-wiki/source/history-pilot/` retain raw responses, metadata, decoded bodies, and calibration; they establish acquisition and decoding only, not target import behavior.

## Known gaps (current cycle)

- [x] Verify homepage revision-list pagination and IDs: 12 pages of 20 rows cover revisions 0–239 exactly once; all 240 rows have numeric author/date fields, five source author profiles were captured, seven metadata-change diff responses were captured, and all 240 bodies were captured. This establishes only that page's listed history.
- [ ] Acquire every required page's authorized revisions and establish complete counts/pagination. Current latest-revision positions total 45,367 across 6,090 accepted pages plus two unresolved pages; this is a position estimate, not an actual revision inventory.
- [ ] Resolve source-backed old title, slug, tag, and author-profile timestamp transitions.
- [ ] Review native backfill foreign-key and non-content-event constraints before replacing the technical revision 0; straight append conflicts with the native sequential requirement.
- [ ] Build and prove a reversible local multi-revision backfill pilot, including current-body hash equality and bounded queue impact.
- [ ] Establish source-backed creator and ACL dependencies before claiming permission parity.

## Out of scope

- Source cutover or deletion.
- API-key dependency.
- Engine change.
- Rendering work owned by the user.
- Choosing an implementation, identifier-rekeying policy, deletion policy, or replacement/backfill policy before review.
