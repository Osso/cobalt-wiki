# Cobalt metadata export

`tools/cobalt_migration/metadata_export.py` acquires allowlisted metadata for the ordered canonical names supplied by a [listing checkpoint](cobalt-listing-export.md), without storing response HTML or session data.

## What it must do

- [x] Accept `export_metadata(source_origin, fullnames, checkpoint_path, fetch)` with `fullnames` from the listing checkpoint. Fetch sequential paths formed as `/` plus `quote(name, safe=':')`; preserve parser-confirmed canonical identities and map archive keys forward by replacing colons with underscores. Reject duplicate names and archive-key collisions.
- [x] Persist one atomic owner-owned 0600 JSON checkpoint in an owner-only directory: `schema`, `source_origin`, `names_sha256`, `completed_position`, `records`. Resume only the uncompleted suffix; validate origin, ordered-input digest, position and record identities before fetching.
- [x] Store accepted parser fields with `status: accepted` and `archive_key`. Store recognized denied/not-found pages as explicit `status: denied` or `status: not_found` records containing only fullname and archive key; these advance the completed position but are not accepted metadata.
- [x] Catch only `SourcePageRedirect` as an unresolved redirect outcome: persist exactly `fullname`, `archive_key`, and `status: redirect`, advance, and accept that record on resume. Never infer title, ID, tags, target identity, or a source Redirect module from an HTTP redirect. No redirect following or redirect-metadata retrieval.
- [x] Leave the failing page uncompleted on unexpected parser errors, identity mismatches, permanent HTTP failures, exhausted transient retries, unexpected transport errors or atomic-write failure.
- [x] Space GET requests by at least one second within each invocation. Reuse listing acquisition's four-attempt transient retry policy, exponential backoff, jitter and Retry-After handling without changing that policy.

## How it works

- [Listing acquisition contract](cobalt-listing-export.md)
- [Metadata parser contract](cobalt-page-metadata.md)

## Implementation inventory

- `tools/cobalt_migration/metadata_export.py`: canonical-name validation, resumable acquisition and classified records.
- `tools/cobalt_migration/listing_export.py`: unchanged FetchResponse, retry and protected atomic-file helpers.
- `tools/cobalt_migration/page_metadata.py`: unchanged allowlisted metadata extraction and identity validation.

## Tests asserting this spec

`tests/cobalt_migration/test_metadata_export.py`: 12 targeted synthetic tests, including 28 accepted pages followed by a redirect, a later failure, and checkpoint resume; including two distinct pages, a failing second page followed by resume, exact metadata fields, mismatched inputs, denied/missing outcomes, retry delays and atomic-write failure.

## Known gaps (current cycle)

- [ ] Redirect records remain unresolved metadata, not accepted pages. Main owns live redirect acceptance and any future metadata retrieval scope.
- [ ] Main-owned live acquisition and integration acceptance; synthetic callback tests do not prove authenticated browser operation or full-source reconciliation.
- [ ] Independent final verification.

## Out of scope

Browser control, CLI wiring, credentials, source writes, ACL export, importer integration and deployment belong to separate acquisition/integration work. The caller supplies canonical names; this exporter never reconstructs names from archive keys. Request spacing across separate invocations is caller-owned.
