# Cobalt replica status

This page records current proof boundaries for the Cobalt replica. It is not a deployment runbook and does not authorize source-site changes.

## Current components

| Component | Proven | Not proven |
|---|---|---|
| [Native backup inventory](cobalt-backup-inventory.md) | Synthetic behavior and a protected real-archive manifest | Canonical page identity or target import |
| [Form schema library](../../specs/cobalt-data-form-schema.md) | Pure synthetic unit behavior for template splitting, schema/value preservation, and narrow `@@` normalization | Deepwell, renderer, editor, database, query, or UI integration |
| [Page metadata parser](../../specs/cobalt-page-metadata.md) | Pure synthetic HTML parsing for the observed metadata shape | Authenticated real-response acceptance and bulk acquisition |
| [Native packages](cobalt-native-packages.md) | Fixed pnpm dependency fetch | Full package realization, runtime, service, and deployment |

## Source-data limits

Native archive paths retain export keys, not canonical fullnames: underscore conversion does not establish original slugs, titles, tags, authors, or categories. Creation timestamps, authorship, revision history, forum history, and user-account mapping are not exported or reconstructed by current tooling.

The legacy NPC template's apparent invalid `orc: Orc:` YAML remains unrepaired. No saved NPC record is present in the archived source inventory.

## Authenticated metadata boundary

The Wikidot API is disabled with its original settings. The user chose to continue without an API key. Current metadata tooling therefore must not prompt for a key, enable the API, or claim API-derived fields. Authenticated browser acquisition still requires real-response acceptance before it can map archive records.

## Native-build boundary

The first full native package realization reached Deepwell and exposed stable-Rust errors from `assert_matches` and `debug_assert_matches`. Commit `f737c0c` replaces those calls with stable `matches!` assertions. The complete native package build has not yet been retried after that commit; no service, database, object storage, hostname, Cloudflare route, or deployed wiki is proven.
