# Additive World Anvil import

`tools/cobalt_migration/worldanvil_import.py` provides create-only primitives. The requested migration remains **all missing Cobalt wiki pages**, preserving every existing destination article. Player conversion supports plain text and explicitly resolved portrait references. Portrait bytes were uploaded separately through the authenticated browser; other source media remains blocked.

## Current capability matrix

| Source capability | Status | Proof / boundary |
| --- | --- | --- |
| Existing destination articles | Supported | A matching normalized title, slug, or `cobalt-source:<fullname>` tag records `existing`; no create request follows. Local behavioral test. |
| Player `whoAmI`, `rpPrefs`, `contactPrefs` text | Supported | Converts plain text to Plutarch section headings and paragraphs in a private article. Local behavioral test. |
| Player `nicknames`, `pronouns`, `battleTag`, `discordUsername`, `timezone` text | Supported | Converts plain text to sidebar definitions. Local behavioral test. |
| Player source tags | Supported | Retains source tags and adds `player` plus `cobalt-source:<fullname>`. Local behavioral test. |
| Create journal and readback | Supported | Writes pending before create; uncertain responses block another create; a readback mismatch remains `created_unverified`. Local behavioral test. |
| Player portraits with resolved references | Supported in payload conversion | A nonempty source `portrait` requires a caller-supplied positive World Anvil image ID or an http(s) URL exactly matching the source field. Emits `[img:ID|none]` or `[img:URL|none]` before sidebar definitions; rejects missing, unrequested, malformed, or BBCode-delimiter-bearing references. Local behavioral tests. |
| Seven resolved player-portrait uploads | Observed separate browser operation | Authenticated World Anvil `globaluploader` accepted batches of at most 10 files and 10 MiB. The public API has no binary-upload implementation. Creation defaulted public; only the seven new image IDs were PATCHed private through the public API. |
| Other image/media content | Blocked | No general upload, download, linking, or source-media conversion is implemented. |
| Wikidot-style markup in supported player text | Blocked | Detected formatting blocks conversion rather than being transformed or discarded. |
| Unknown nonempty player fields | Blocked | Conversion fails rather than dropping data. |
| Characters, background characters, writings, reference pages, templates, dynamic pages, and all other categories | Missing | No converter is implemented. |
| Cross-page links and attachment preservation | Missing | No conversion or upload/link workflow is implemented. |
| Destination-wide identity reconciliation | Partial | Title/slug and archive-ID audits exist; manual profile-name review found additional aliases. Remaining candidates are not proven absent. |
| Pre-existing article preservation | Observed for first four additions | All 175 baseline article records retained identical article-owned fields; only nested world/category update timestamps changed. |

## Source and execution status

- Audit inventory: 6,092 source pages across 18 namespaces and 1,471 media files.
- Verified September 24, 2026: created four private player articles—Anakin, Barry, E, Sharksu—in the existing Players category. Live inventory increased from 175 to 179; readbacks matched submitted content, sidebar, tags, privacy, and category.
- Portrait-upload evidence: `/home/osso/.local/share/cobalt-wiki/worldanvil/import-20260924/upload-journal.json` records the seven new image IDs and archived source hashes. All seven uploaded bytes were downloaded and hash-matched to their archived originals. Browser authentication was required; no source credentials were used or stored.
- The upload operation did not modify pre-existing images or pages. The seven newly created images were made private, but their public CDN URLs remained anonymously readable; `image.state=private` is not a CDN privacy guarantee.
- Player article creation using those portraits has not run yet. Independent slice verification passed 16 focused tests and checked all four created readbacks plus 175 pre-existing records. Full migration is not complete.
- The full all-missing-pages migration remains open; this module is not an importer for the full source inventory.

## How it works

- [Create-only transport contract](cobalt-worldanvil-client.md)
- The caller supplies source identity, converted payload, and journal path.
- A pending or unverified creation requires reconciliation, not another create request.

## Implementation inventory

- `tools/cobalt_migration/worldanvil_import.py`: plain-text player payload conversion with optional resolved portrait, destination identity check, creation journal, readback checks.
- `tools/cobalt_migration/worldanvil_client.py`: read and create transport; no binary media upload implementation.

## Tests asserting this spec

- `tests/cobalt_migration/test_worldanvil_import.py`: profile content/privacy, explicit unsupported-source failures, preservation, uncertain-response resume, readback mismatch.
- `tests/cobalt_migration/test_worldanvil_client.py`: local HTTP pagination and create-only effects.

## Known gaps (current cycle)

- [ ] All non-player page-category conversions are unimplemented.
- [ ] General media upload and linking beyond the seven separately uploaded player portraits are unimplemented.
- [ ] Wikidot markup conversion is unimplemented.
- [ ] Unknown player fields need explicit mappings before import.
- [ ] No ongoing synchronization is implemented.

## Out of scope

- Updating or deleting pre-existing articles: explicitly prohibited by the user.
- Deploying the local Cobalt replica or installing a change-sync hook: separate work.
