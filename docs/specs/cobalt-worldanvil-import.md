# Additive World Anvil import

> **Paused — September 25, 2026.** No automatic World Anvil hook was installed. Manual import is stopped; do not publish further World Anvil content without a new user request. The retained helpers and partial data do not make the full migration complete.
>
> Local operation journals record 54 created articles (11 players, 2 characters, and 41 technical references), 1 rejected item, and 321 hash-verified image uploads. These are operation records, not a fresh remote inventory.

`tools/cobalt_migration/worldanvil_import.py` provides create-only primitives. The user-selected goal remains **all missing Cobalt wiki pages** while preserving existing destination articles; it is not complete.

## Current capability matrix

| Capability | Status | Boundary |
| --- | --- | --- |
| Caller-owned bounded-batch inventory | Supported | Caller lists once, indexes it, and passes the shared index to every `import_page`; no per-page full refetch. Successful creates enter that index before readback. |
| Existing-article detection | Supported | Matches normalized planned payload title and `cobalt-source:<fullname>` marker. Namespace short aliases additionally apply only to `player`, `character`, and `bgc`. Multiple candidates block creation. |
| Same-name applications and players | Supported | An application may create separately from an existing same-name player because matching uses its planned title and no application short alias. |
| Player text | Supported | `whoAmI`, `rpPrefs`, and `contactPrefs` become private-body sections; `nicknames`, `pronouns`, `battleTag`, `discordUsername`, and `timezone` become sidebar definitions. Unknown nonempty fields and Wikidot-style markup block conversion. |
| Resolved player portraits | Supported in payload conversion | A nonempty source portrait requires a positive World Anvil image ID or an exact matching `http(s)` source URL; emits `[img:reference|none]`. |
| Source-reference payloads | Supported | Exact raw source is stored in `authornotes`; body is explanatory. Readback compares `authornotes` when present. |
| Other page categories/media/link preservation | Missing | No general converter/upload/link workflow is implemented. |

## Execution evidence and limits

- 321 new images were uploaded and hash-verified: 8 initial uploads plus 313 profile uploads.
- The baseline 175 articles were untouched as of the proof covering 11 player additions. This is not a claim that all later work has a new full-preservation proof.
- Current owned additions include Abigael and Addelaine plus navigation-side and admin-CSS source references.
- Native creation of Addelaine succeeded after scrolling the create button into the viewport. The earlier failed click was a CLI offscreen interaction failure, not evidence of a World Anvil platform bug.
- The full all-missing-pages migration remains open.

## How it works

1. Caller obtains one complete live inventory for a bounded batch using `client.list_articles(world_id)` and calls `index_articles(articles)`.
2. Caller passes that shared index to `import_page(client, world_id, source, payload, journal_path, inventory)` for each page. The importer does not list articles.
3. A source marker or supported title match records `existing`; a pending or unverified journal entry requires reconciliation, never another create request.
4. A successful create is added to the in-memory index, then read back. Mismatched fields, including `authornotes` when supplied, remain `created_unverified`.

The snapshot is not atomic remote create-if-absent; another writer can create after inventory fetch. Reconcile that race externally. This module never updates existing articles.

## Implementation inventory

- `tools/cobalt_migration/worldanvil_import.py`: player payload conversion, inventory identity check, creation journal, and readback.
- `tools/cobalt_migration/worldanvil_reference.py`: private technical source-reference payloads.
- `tools/cobalt_migration/worldanvil_client.py`: read/create transport only.

## Tests asserting this spec

- `tests/cobalt_migration/test_worldanvil_import.py`: profile fields, portrait boundaries, existing preservation, batch index behavior, title/marker identity, application/player distinction, uncertain responses, and readback mismatches including missing `authornotes`.

## Known gaps

- [ ] Non-player categories, rendered-content integration, general media, cross-page links, and attachment preservation remain unimplemented.
- [ ] Unknown player fields need explicit mappings.
- [ ] No ongoing synchronization exists.

## Out of scope

- Updating or deleting pre-existing articles, deployment, and change synchronization.
