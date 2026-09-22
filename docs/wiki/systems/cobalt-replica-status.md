# Cobalt replica status

This page records current proof boundaries for the Cobalt replica. It is not a deployment runbook and does not authorize source-site changes.

## Current components

| Component | Proven | Not proven |
|---|---|---|
| [Native backup inventory](cobalt-backup-inventory.md) | Synthetic behavior and a protected real-archive manifest | Canonical page identity or target import |
| [Form schema library](../../specs/cobalt-data-form-schema.md) | Pure schema/value behavior; bounded Deepwell page-view payload and authorized whole-record edit wiring | Frontend editor, rendering/query parity, DB-backed behavior, and complete workflow integration |
| [Page metadata parser](../../specs/cobalt-page-metadata.md) | 30 synthetic tests; protected tagged, untagged, and NBSP-tagged responses | Full acquisition, other live response shapes, and import |
| [Listing export](../../specs/cobalt-listing-export.md) | Protected authenticated run: 277/277 listing pages, 6,092 literal fullnames, forward reconciliation to all 6,092 archive source keys with zero gaps/collisions | Metadata/ACL acquisition, import, and deployment |
| [Native packages](cobalt-native-packages.md) | Deepwell, WWS, Framerail, and Silo realize; native runtime/bootstrap deployed privately; loopback gateway rejects unauthenticated requests | Authenticated WWS file routing, full-archive import/reconciliation, public ingress, source ACL parity |

## Source-data limits

Native archive paths retain export keys, not canonical fullnames. The protected listing establishes a forward-only reconciliation: replacing each canonical colon with an underscore maps all 6,092 listed names to archive source keys without gaps or collisions. This does not permit inverse underscore-to-colon reconstruction or establish titles, tags, authors, or categories. Creation timestamps, authorship, revision history, forum history, and user-account mapping are not exported or reconstructed by current tooling.

The legacy NPC template's apparent invalid `orc: Orc:` YAML remains unrepaired. No saved NPC record is present in the archived source inventory.

## Authenticated metadata boundary

The Wikidot API is disabled with its original settings. The user chose to continue without an API key. Current metadata tooling therefore must not prompt for a key, enable the API, or claim API-derived fields. Protected tagged, untagged, and NBSP-tagged parser fixtures passed independently. Listing export completed a protected authenticated 277-page run with 6,092 literal canonical fullnames; its forward reconciliation to the 6,092 archive source keys has zero gaps or collisions. Bulk metadata acquisition is ongoing: accepted records are distinct from explicit `denied` and unresolved `redirect` records, and none of those outcomes proves ACL export or target import.

## Privacy boundary

The permission model supports virtual member/category roles and a page-author role when a page reference is supplied. Current page views use `page_reference: None`, so they do not prove creator-specific behavior. Current WWS attachment routes do not enforce page-view authorization; private attachments must not be exposed until that route/session boundary is implemented and behaviorally verified.

## Native-build and POC boundary

Commit `f737c0c` replaces the stable-Rust-incompatible assertion macros. Deepwell, WWS, and Framerail then realized successfully; proof is `/tmp/claude/cobalt-native-package-build-fixed.log`. Commit `aeb81fe` adds pinned Silo and an isolated Go 1.27.1 toolchain. Agent44 independently realized Silo at `/nix/store/am512fba05178mdfzlzsngd3anz7w1xb-silo-2026-09-16`; enabled runtime-module evaluation passes and reports Silo `DEVELOPMENT.GOGET` on Go 1.27.1.

On September 22, 2026, the native runtime bootstrapped site `6000000`. The gateway's 29 unauthenticated/wrong-password route and method checks, including an asset, returned `401`. Public HTTPS `cobalt-company.sakuin.org` now reaches the loopback gateway through the existing Sakuin Cloudflare Tunnel as a proxied CNAME; browser-like requests receive its Basic challenge, while Python's default client is blocked upstream with Cloudflare `1010`. Runtime fixes deployed in closure `1rha1gh6jyvp39vfw608n0bfj6fbf3aq`; Sakuin readiness passed. The initial import stopped on detected ordinary page-creation slug normalization and the importer-owned wrong target was soft-deleted. A fresh import is active, with no completed reconciliation counts yet. The positional `site_domain` fix makes an authenticated missing-file route return `404`, but authenticated `robots.txt` still returns `502`. Source ACL parity, attachment authorization, full reconciliation, and rendering compatibility remain unproven.
