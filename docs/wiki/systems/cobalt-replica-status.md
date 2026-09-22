# Cobalt replica status

This page records current proof boundaries for the Cobalt replica. It is not a deployment runbook and does not authorize source-site changes.

## Current components

| Component | Proven | Not proven |
|---|---|---|
| [Native backup inventory](cobalt-backup-inventory.md) | Synthetic behavior and a protected real-archive manifest | Canonical page identity or target import |
| [Form schema library](../../specs/cobalt-data-form-schema.md) | Pure synthetic unit behavior for template splitting, schema/value preservation, and narrow `@@` normalization | Deepwell, renderer, editor, database, query, or UI integration |
| [Page metadata parser](../../specs/cobalt-page-metadata.md) | 25 synthetic tests and one protected authenticated `character:atley` response | Bulk acquisition, other live response shapes, and import |
| [Native packages](cobalt-native-packages.md) | Deepwell, WWS, and Framerail realize at `f737c0c`; isolated Go 1.27.1 realizes | Silo realization, client compatibility/security, runtime, service, and deployment |

## Source-data limits

Native archive paths retain export keys, not canonical fullnames: underscore conversion does not establish original slugs, titles, tags, authors, or categories. Creation timestamps, authorship, revision history, forum history, and user-account mapping are not exported or reconstructed by current tooling.

The legacy NPC template's apparent invalid `orc: Orc:` YAML remains unrepaired. No saved NPC record is present in the archived source inventory.

## Authenticated metadata boundary

The Wikidot API is disabled with its original settings. The user chose to continue without an API key. Current metadata tooling therefore must not prompt for a key, enable the API, or claim API-derived fields. The parser accepted one protected authenticated `character:atley` response at `bec4ba1`; this proves that response only. Canonical listing acquisition, bulk metadata collection, ACL export, and archive-record mapping remain unproven.

## Native-build boundary

Commit `f737c0c` replaces the stable-Rust-incompatible assertion macros. Deepwell, WWS, and Framerail then realized successfully; proof is `/tmp/claude/cobalt-native-package-build-fixed.log`. Commit `aeb81fe` adds pinned Silo and an isolated Go 1.27.1 toolchain. Go realizes, but the prior Silo compile stopped for disk exhaustion; its current realization, executable installation, client compatibility/security, runtime evaluation, service, database, object storage, hostname, Cloudflare route, and deployed wiki remain unproven.
