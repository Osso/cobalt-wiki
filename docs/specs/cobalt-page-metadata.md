# Cobalt page HTML metadata

`tools/cobalt_migration/page_metadata.py` extracts a minimal metadata record from one Wikidot page response. It performs no I/O and does not execute JavaScript. Authenticated fetching and protected persistence belong to the caller; this parser does not enable or use the site API.

## What it must do

- [x] `parse_page_metadata(html, expected_fullname=None)` returns exactly `fullname`, `page_id`, `title`, `tags`, `revision_number`, and `updated_at`.
- [x] Read canonical identity only from literal inline JavaScript assignments to `WIKIREQUEST.info.pageUnixName` and `WIKIREQUEST.info.pageId`. Accept JSON strings and single-quoted strings with explicit quote, slash, backslash, control, hexadecimal, and UTF-16 escapes. Reject invalid fullname characters, isolated surrogates, nonpositive IDs, expressions, and conflicting repeated assignments.
- [x] Ignore assignments mentioned in comments, quoted strings, template-literal text, non-JavaScript script elements, and ordinary article text. Return no other script properties or source contents.
- [x] Extract visible title text from `#page-title`, preserving Unicode/entities and nested markup text. Exclude script/style/template contents.
- [x] Extract tags from `.page-tags a`, retaining hidden-tag spelling such as `_completed`, deduplicating and sorting exact decoded tag text. A present empty tag container means no tags; a missing container or blank tag is an error.
- [x] Read the current revision from `#page-info` text and `updated_at` from its `.odate` element's `time_<epoch>` class, never from localized date text or article timestamps.
- [x] Reject missing, duplicate, malformed, or ambiguous required metadata. Require exact canonical fullname agreement when `expected_fullname` is supplied.
- [x] Raise `SourcePageUnavailable` with `reason="denied"` or `reason="not_found"` for recognized denial/missing-page titles or source messages, even when a fetch returned HTTP 200.
- [x] Raise `PageMetadataError` for other extraction failures without echoing HTML, private prose, nonces, or conflicting scalar values.

## How it works

- [Parser and public exceptions](../../tools/cobalt_migration/page_metadata.py)
- [Behavioral fixtures](../../tests/cobalt_migration/test_page_metadata.py)
- [Separate raw-archive inventory contract](cobalt-backup-inventory.md)

Identity parsing is limited to the observed dot-property assignments, with literal values followed by a semicolon or script end. Single-quoted escapes support `\'`, `\"`, `\\`, `\/`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v`, `\xHH`, and `\uHHHH`; whitespace/control characters remain invalid in a fullname. This is not JavaScript evaluation or general program analysis. Computed property names, expression-valued metadata, interpolated template metadata, and automatic-semicolon-insertion variations are not supported.

Unavailable-page recognition is deliberately bounded to `Private Content`, `Access denied`, `Permission denied`, `Page not found`, and `Page does not exist` titles, or page-content messages beginning `This area of the site is private` / `The page you want to access does not exist`. Other response shapes must still establish all required metadata or fail explicitly; this is not a general HTTP/authentication classifier.

## Implementation inventory

- `tools/cobalt_migration/page_metadata.py` — stdlib HTML collection, allowlisted literal decoding, consistency checks, public pure parser/errors.
- `tests/cobalt_migration/test_page_metadata.py` — synthetic HTML behavior tests; no network or private fixtures.

## Tests asserting this spec

`tests/cobalt_migration/test_page_metadata.py`: 20 targeted tests pass, including conflicting identities, unsupported expressions, escaped Unicode, hidden tags, footer scoping, denial/missing responses, empty versus missing metadata, and omission of unrelated script data.

```text
python -B -m unittest discover -s tests/cobalt_migration -p test_page_metadata.py -v
```

## Known gaps (current cycle)

- [ ] Caller integration and authenticated real-response acceptance are not part of this slice; only the supplied observed HTML shape has synthetic proof.
- [ ] Other Wikidot localization, denial wording, or identity-assignment syntax needs source evidence and tests before support can be claimed.

## Out of scope

- Authenticated fetching, retries, credentials, cookies, API configuration, and persistence: caller-owned.
- Creation timestamps, authorship, historical revisions, permissions, and API authentication: not established by these fields; never fabricated.
- Archive filename/category reconstruction: raw archive keys remain separate; the optional expected fullname verifies a caller's candidate rather than generating one.
- Source-site edits, deployment, and broad migration acceptance.
