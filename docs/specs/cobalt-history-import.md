# Cobalt history import

Cobalt history import must store authorized source revision history without presenting the latest-source migration as complete history. The current migration has 6,092 pages and 1,471 files. The user approved importing historical text from Wikidot's display representation even where tabs may become spaces; raw responses and that limit must remain recorded. The bounded homepage pilot acquired 240 listed revisions and all 240 source-module representations; it did not establish byte-exact historical bodies or whole-site history counts. See the [current POC import contract](cobalt-poc-import.md), the [native revision import endpoint](../../deepwell/src/endpoints/import.rs), the [native import service](../../deepwell/src/services/import/service.rs), and the [stock WikiComma importer entry point](../../deepwell/importer/__main__.py).

## What it must do

### Authorized source acquisition

- [ ] Acquire only authorized, available source revision records: source IDs, per-page numbers, timestamps, author IDs, comments, revision-specific metadata, raw source-module responses, and their display-decoded bodies.
- [ ] Preserve the raw response and mark each decoded historical body as display-decoded, not byte-exact. The approved tab-to-space limit does not apply to the current archived source, which remains byte-exact.
- [ ] Record unavailable or inaccessible revisions as explicit gaps; do not infer, synthesize, or silently omit them.
- [ ] Keep source creator and ACL dependencies explicitly unknown until authorized source evidence establishes them.
- [ ] Avoid an API-key dependency for acquisition or import.
- [ ] Archive each authorized page's revision lists before source bodies; persist owner-only raw and parsed responses before advancing its checkpoint, then resume without refetching verified records.
- [ ] Classify unavailable body responses and unobserved revision-number ranges as explicit gaps. Stop on unavailable or inconsistent revision-list responses; never reinterpret them as missing history.
- [ ] Bind page and site checkpoints to the requested source origin, source page identity, protected inventory, and response hashes. Use sequential one-second request spacing and at most four retry attempts for transient failures, respecting `Retry-After`.

### Imported history integrity

- [x] Preserve the current imported page and latest source while imported history is stored separately.
- [ ] Preserve source revision order, source identifiers, numbers, timestamps, author identities, comments, and revision-specific metadata when acquired.
- [ ] Never fabricate revision records, bodies, author identities, or metadata.
- [ ] Distinguish the technical migration record already present on POC pages from source history; do not represent it as a source revision.
- [ ] Reconcile every imported record and explicitly report retained gaps after import.

### Proof and operational safety

- [x] Acquire all 240 revision-list records (numbers 0–239) and all 240 historical source-module responses for one page through the authenticated read-only source UI.
- [x] Decode all 240 captured source-module representations with explicit display-decoded provenance; revision 239 decodes to the archived source SHA-256 `eb0478369cf1cae46acb85b994c77162e3116ce7694ca7c5399ae26e46890beb`.
- [x] Confirm that the module returns HTML-wrapped source representations rather than an independently verified byte-identical archive representation.
- [x] Store three source records idempotently in local imported-history storage without changing the current revision, compiled output, or native revision count.
- [x] Submit the acquired 240-record homepage history set (`0`–`239`) once to main local site `6000000` through `import_wikidot_history`. The protected preflight bound `home:start` page `3000000141`, source page `1310927108`, guarded current revision `3000000281`, and a zero-row starting inventory; the import response reported `{"inserted":240}`. Reconciliation remains pending in the [replica status](../wiki/systems/cobalt-replica-status.md#imported-source-history).
- [x] Reconcile the current-body comparison: revision 239's decoded payload and local API body are 2,473 bytes with SHA-256 `eb0478369cf1cae46acb85b994c77162e3116ce7694ca7c5399ae26e46890beb`, matching the archived current source. The older unequal comparison artifact predates decoder correction and is retained as superseded evidence.
- [ ] Keep historical display-decoding provenance and raw responses through import. Do not promise byte-exact recovery for old bodies.
- [ ] Prevent historical bulk work from causing an outdate or rerender flood; validate queue impact before production work.
- [ ] Require review before any operation that changes native editable revisions, page identity, or source deletion policy.

## How it works

- [POC import](cobalt-poc-import.md)
- [Queue incident and recovery](../wiki/systems/cobalt-queue-recovery.md)
- [Stock WikiComma importer implementation](../../deepwell/importer/importer.py)
- [Stock WikiComma archive reader](../../deepwell/importer/site.py)
- [Native revision import endpoint](../../deepwell/src/endpoints/import.rs)
- [Native revision import service](../../deepwell/src/services/import/service.rs)
- [Current imported-history state and boundaries](../wiki/systems/cobalt-replica-status.md#imported-source-history)

## Local backend history storage

Imported source history uses `imported_page_revision`, separate from native editable revisions. Unknown historical title/slug/tags remain null; source attachment events retain their original flags. Import does not change the current revision or render pages. Read endpoints enforce current page visibility; imported entries are not native rollback targets.

- [x] Import three source records idempotently without changing current source, compiled output, or native revision count.
- [x] Read history across two cursor pages and retrieve an old body with its source author ID and representation marker.
- [x] Reject changed existing source records, duplicate revision identities, wrong-site targets, and a stale current-page revision without replacing current content.
- [x] Enforce current-page visibility using trusted request identity; JSON `user_id` cannot override it.
- [x] Preserve imported bodies during normal text pruning while removing an unreferenced text fixture.
- [x] Import and replay history for a navigation-page fixture without enqueueing rerenders.
- [x] Independently verify the integrated history backend and local pilot at `9c4ff20`: six history tests and one prune test passed; the existing isolated queue test remains valid at one passing test. Format and cargo check passed.
- [x] Run the acquired 240-revision pilot through this storage path: seven cursor pages read 240 matching bodies and metadata; idempotent replay inserted zero records. This remains isolated site `6000011` evidence, not main-local or production proof.
- [x] Submit the same acquired set once to main local backend `2749` / site `6000000`; the protected response reported 240 inserted records. The preflight current-body identity was 2,473 bytes and SHA-256 `eb0478369cf1cae46acb85b994c77162e3116ce7694ca7c5399ae26e46890beb`, matching the protected main plan. Exact before/after preservation, 240-body raw readback, and browser verification remain pending; see the [replica status](../wiki/systems/cobalt-replica-status.md#imported-source-history).
- [x] Prepare an isolated local history-pilot Vite configuration at `301ae8f84`: it reuses the main Vite config exports and SvelteKit's supported flat plugin options while writing Vite cache to `.vite-history-pilot` and Kit output to `.svelte-kit-history-pilot`; both paths are ignored. Main canonical configuration remains unchanged.
- [x] Install the pilot-only user-service override at `/home/osso/.config/systemd/user/cobalt-local-framerail.service.d/10-history-pilot.conf`, selecting `vite.history-pilot.config.ts` on `5173`, and retain the GREEN browser proof at `2113c00b1`: 240 metadata rows across five pages, 239/0 body hashes, read-only/provenance/no-rollback behavior, allowlisted native and imported-history reads only, current-body hash and reload. The prior `0db5c2fc0` hydration failure and post-isolation selector correction `49f789a64` are historical; `2113c00b1` corrects an erroneous block on native history reads. Predicate extraction at `89550f5c3` is origin/path/action-equivalent, so this proof remains valid. Final verifier `684` passed ESLint with zero findings, Prettier, and Node syntax checks; applicable function metrics remain below threshold (maximum cognitive `13`, cyclomatic `15`; extracted helper `1`/`3`). Its aggregate unit-complexity `22` is not a function metric and is rejected under the skill's literal function-only threshold. This proves the bounded isolated pilot, not main/production parity, production import, or full replica history; 43 original full hashes remain unreconstructable. See the [replica status](../wiki/systems/cobalt-replica-status.md#imported-source-history).

`import_wikidot_history` accepts a guarded current revision ID and source records. `page_imported_history` lists at most 100 records, descending by source revision number; `before_revision` is exclusive. `page_imported_revision` retrieves a source body. Main and production history parity remain unestablished.

## Implementation inventory

- `deepwell/importer/site.py`: reads revision metadata and per-page `.7z` bodies keyed by revision number.
- `deepwell/importer/importer.py`: writes the stock SQLite/S3 target.
- `deepwell/importer/__main__.py`: importer command entrypoint; it does not crawl source history.
- `deepwell/src/endpoints/import.rs`: exposes `import_wikidot_page_revision`.
- `deepwell/src/services/import/service.rs`: native Wikidot revision import behavior; the endpoint expects a sequential history beginning at revision 0.
- `tools/cobalt_migration/page_history.py`: parses revision-list metadata and gaps.
- `tools/cobalt_migration/history_source.py`: decodes the source display with explicit non-byte-exact provenance.
- `tools/cobalt_migration/history_export.py`: owner-only, resumable archive for one page's list and source-body module responses; lists complete before body capture.
- `tools/cobalt_migration/history_acquire.py`: validates the protected latest-source plan and runs the page exporter sequentially, retaining explicit metadata-unresolved records.
- `tools/cobalt_migration/poc_import.py`: remains latest-source only; no history target write exists yet.
- `framerail/src/routes/[slug]/[...extra]/ImportedHistory.svelte`: presents imported records separately from editable revisions, with source access and display-decoding provenance but no rollback control. Its `c7859d0` browser flow is historical; current UI proof is blocked as recorded in the [replica status](../wiki/systems/cobalt-replica-status.md#imported-source-history).

- `deepwell/src/services/import/history.rs` and `history_structs.rs`: guarded import and permission-checked reads.
- `deepwell/migrations/20260923000000_imported_page_revision.sql`: separate source-history storage.

## Tests asserting this spec

- `deepwell/tests/imported_history.rs`: native import/idempotence/current-page preservation and real two-page history reads.

`tests/cobalt_migration/test_page_history.py` has nine independently verified parser tests. `tests/cobalt_migration/test_history_source.py` has five decoder tests; all 240 protected pilot responses decode. `tests/cobalt_migration/test_history_export.py` has eight synthetic tests for resumable list/body archival, protected checkpoint validation, gaps, and bounded failures. `tests/cobalt_migration/test_history_acquire.py` covers protected-plan site orchestration. These tests do not establish source-wide acquisition or live browser transport. Independent verification at `9c4ff20` passed six history tests, one prune test, the retained isolated queue test, format, and cargo check; its prior abnormal scoped run lacked `DATABASE_URL`. Protected artifacts under `/home/osso/.local/share/cobalt-wiki/source/history-pilot/` retain raw responses, metadata, decoded bodies, calibration, and local import/readback proofs. Current API/UI evidence and deployment boundaries are maintained in the [replica status](../wiki/systems/cobalt-replica-status.md#imported-source-history). This does not establish production import, current local UI access, site-wide acquisition, or byte-exact recovery of older historical bodies. The user alone decides whether any future deployment is warranted; no deployment occurs without explicit approval.

## Known gaps (current cycle)

- [x] Verify homepage revision-list pagination and IDs: 12 pages of 20 rows cover revisions 0–239 exactly once; all 240 rows have numeric author/date fields, five source author profiles were captured, seven metadata-change diff responses were captured, and all 240 bodies were captured. This establishes only that page's listed history.
- [ ] Acquire every required page's authorized revisions and establish complete counts/pagination. The protected plan has 6,090 accepted source-page identities and two metadata-unresolved records; latest-revision positions total 45,367 across accepted pages. This is a position estimate, not an actual revision inventory.
- [ ] Wire the pending target-pinned browser transport to `history_acquire.py` and run only authorized, checkpointed source acquisition. The CLI interface is `--plan`, `--source-origin`, `--archive-directory`, `--target-id`, and `--node-binary`; it is not yet evidence of a working live transport.
- [ ] Resolve source-backed old title, slug, tag, and author-profile timestamp transitions.
- [x] Import the acquired 240-revision homepage pilot through separate storage and reconcile it locally: site `6000011` has 240 records, seven read cursor pages, matching bodies/metadata, and zero replay insertions; current revision/source/compiled output unchanged. The separate navigation-fixture test established zero rerender enqueueing.
- [x] Submit the acquired homepage set once to main local site `6000000`; protected preflight identifies `home:start` page `3000000141`, source page `1310927108`, guarded current revision `3000000281`, and zero existing imported rows, while the response reports 240 inserted records.
- [ ] Reconcile the main-local import: exact current native revision/source/draft preservation, all 240 stored raw bodies and metadata, and browser behavior. Native revisions, current source, and drafts were outside the data-import operation; do not claim preservation before comparison.
- [ ] Review any future native editable-revision replacement separately; it is not required for imported source history.
- [ ] Establish source-backed creator and ACL dependencies before claiming permission parity.

## Out of scope

- Source cutover or deletion.
- API-key dependency.
- Engine change.
- Rendering work owned by the user.
- Rewriting native editable revision history, identifier rekeying, or source deletion.
