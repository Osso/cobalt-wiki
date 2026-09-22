# Cobalt replica status

This page records current proof boundaries for the Cobalt replica. It is not a deployment runbook and does not authorize source-site changes.

## Current components

| Component | Proven | Not proven |
|---|---|---|
| [Native backup inventory](cobalt-backup-inventory.md) | Synthetic behavior and a protected real-archive manifest | Canonical page identity or target import |
| [Form schema library](../../specs/cobalt-data-form-schema.md) | Pure schema/value behavior; bounded Deepwell page-view payload and authorized whole-record edit wiring | Frontend editor, rendering/query parity, DB-backed behavior, and complete workflow integration |
| [Page metadata parser](../../specs/cobalt-page-metadata.md) | 30 synthetic tests; protected tagged, untagged, and NBSP-tagged responses | Full acquisition, other live response shapes, and import |
| [Listing export](../../specs/cobalt-listing-export.md) | Protected authenticated run: 277/277 listing pages, 6,092 literal fullnames, forward reconciliation to all 6,092 archive source keys with zero gaps/collisions | Metadata/ACL acquisition, import, and deployment |
| [Native packages](cobalt-native-packages.md) | Deepwell, WWS, Framerail, and Silo realize; enabled runtime module evaluates with Silo `DEVELOPMENT.GOGET` on Go 1.27.1 | Client compatibility/security, running services, integration, and deployment |

## Source-data limits

Native archive paths retain export keys, not canonical fullnames. The protected listing establishes a forward-only reconciliation: replacing each canonical colon with an underscore maps all 6,092 listed names to archive source keys without gaps or collisions. This does not permit inverse underscore-to-colon reconstruction or establish titles, tags, authors, or categories. Creation timestamps, authorship, revision history, forum history, and user-account mapping are not exported or reconstructed by current tooling.

The legacy NPC template's apparent invalid `orc: Orc:` YAML remains unrepaired. No saved NPC record is present in the archived source inventory.

## Authenticated metadata boundary

The Wikidot API is disabled with its original settings. The user chose to continue without an API key. Current metadata tooling therefore must not prompt for a key, enable the API, or claim API-derived fields. Protected tagged, untagged, and NBSP-tagged parser fixtures passed independently. Listing export completed a protected authenticated 277-page run with 6,092 literal canonical fullnames; its forward reconciliation to the 6,092 archive source keys has zero gaps or collisions. Bulk metadata acquisition is ongoing: accepted records are distinct from explicit `denied` and unresolved `redirect` records, and none of those outcomes proves ACL export or target import.

## Privacy boundary

The permission model supports virtual member/category roles and a page-author role when a page reference is supplied. Current page views use `page_reference: None`, so they do not prove creator-specific behavior. Current WWS attachment routes do not enforce page-view authorization; private attachments must not be exposed until that route/session boundary is implemented and behaviorally verified.

## Native-build boundary

Commit `f737c0c` replaces the stable-Rust-incompatible assertion macros. Deepwell, WWS, and Framerail then realized successfully; proof is `/tmp/claude/cobalt-native-package-build-fixed.log`. Commit `aeb81fe` adds pinned Silo and an isolated Go 1.27.1 toolchain. Agent44 independently realized Silo at `/nix/store/am512fba05178mdfzlzsngd3anz7w1xb-silo-2026-09-16`; enabled runtime-module evaluation passes and reports Silo `DEVELOPMENT.GOGET` on Go 1.27.1. Client compatibility/security, service startup, database/object storage behavior, hostname, Cloudflare route, and deployed wiki remain unproven.
