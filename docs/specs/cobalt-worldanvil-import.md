# Additive World Anvil import

`tools/cobalt_migration/worldanvil_import.py` provides create-only primitives. The requested migration remains **all missing Cobalt wiki pages**, preserving every existing destination article. Only text-only player conversion is implemented.

## Current capability matrix

| Source capability | Status | Proof / boundary |
| --- | --- | --- |
| Existing destination articles | Supported | A matching normalized title, slug, or `cobalt-source:<fullname>` tag records `existing`; no create request follows. Local behavioral test. |
| Player `whoAmI`, `rpPrefs`, `contactPrefs` text | Supported | Converts plain text to Plutarch section headings and paragraphs in a private article. Local behavioral test. |
| Player `nicknames`, `pronouns`, `battleTag`, `discordUsername`, `timezone` text | Supported | Converts plain text to sidebar definitions. Local behavioral test. |
| Player source tags | Supported | Retains source tags and adds `player` plus `cobalt-source:<fullname>`. Local behavioral test. |
| Create journal and readback | Supported | Writes pending before create; uncertain responses block another create; a readback mismatch remains `created_unverified`. Local behavioral test. |
| Player portraits and other image/media content | Blocked | A nonempty `portrait` blocks conversion. Upload and destination media linking are not implemented. |
| Wikidot-style markup in supported player text | Blocked | Detected formatting blocks conversion rather than being transformed or discarded. |
| Unknown nonempty player fields | Blocked | Conversion fails rather than dropping data. |
| Characters, background characters, writings, reference pages, templates, dynamic pages, and all other categories | Missing | No converter is implemented. |
| Cross-page links and attachment preservation | Missing | No conversion or upload/link workflow is implemented. |
| Destination-wide identity reconciliation | Partial | Title/slug and archive-ID audits exist; manual profile-name review found additional aliases. Remaining candidates are not proven absent. |
| Pre-existing article preservation | Observed for first four additions | All 175 baseline article records retained identical article-owned fields; only nested world/category update timestamps changed. |

## Source and execution status

- Audit inventory: 6,092 source pages across 18 namespaces and 1,471 media files.
- Verified September 24, 2026: created four private player articles—Anakin, Barry, E, Sharksu—in the existing Players category. Live inventory increased from 175 to 179; readbacks matched submitted content, sidebar, tags, privacy, and category.
- Operation evidence: `/home/osso/.local/share/cobalt-wiki/worldanvil/import-20260924/` contains the creation journal, payload plan, before/after inventories and article details, and `preservation-proof.json`. Eight remaining player candidates have portraits and were not created.
- Independent slice verification passed 16 focused tests and checked all four created readbacks plus 175 pre-existing records. Full migration is not complete.
- The full all-missing-pages migration remains open; this module is not an importer for the full source inventory.

## How it works

- [Create-only transport contract](cobalt-worldanvil-client.md)
- The caller supplies source identity, converted payload, and journal path.
- A pending or unverified creation requires reconciliation, not another create request.

## Implementation inventory

- `tools/cobalt_migration/worldanvil_import.py`: text-only player payload conversion, destination identity check, creation journal, readback checks.
- `tools/cobalt_migration/worldanvil_client.py`: read and create transport; no update/delete operation.

## Tests asserting this spec

- `tests/cobalt_migration/test_worldanvil_import.py`: profile content/privacy, explicit unsupported-source failures, preservation, uncertain-response resume, readback mismatch.
- `tests/cobalt_migration/test_worldanvil_client.py`: local HTTP pagination and create-only effects.

## Known gaps (current cycle)

- [ ] All non-player page-category conversions are unimplemented.
- [ ] Media upload and destination media linking are unimplemented.
- [ ] Wikidot markup conversion is unimplemented.
- [ ] Unknown player fields need explicit mappings before import.
- [ ] No ongoing synchronization is implemented.

## Out of scope

- Updating or deleting pre-existing articles: explicitly prohibited by the user.
- Deploying the local Cobalt replica or installing a change-sync hook: separate work.
