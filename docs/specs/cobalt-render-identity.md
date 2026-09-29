# Cobalt compiled-render identity

Stored HTML is reused while its rendering-input identity remains current. `deepwell/render_identity.rs` supplies the content fingerprint used by the Cargo build script; local Cargo, app deployment and Nix packages use that same calculation. This addresses the first-view build-mismatch cost measured in the [roster profile](../wiki/systems/cobalt-roster-performance.md).

## What it must do

- [x] Equal input contents produce equal identities regardless of checkout path, traversal order, filesystem timestamps or Git metadata.
- [x] Documentation, frontend, tests and build-output changes do not change the identity.
- [x] Backend Rust, vendored dependencies, manifests/lockfile, default configuration, embedded templates and locale changes change the identity. The hash is deliberately conservative: unrelated backend API code can still invalidate HTML; it does not attempt semantic dependency analysis.
- [x] Adding/removing input files changes identity; missing required inputs fail with the affected path. File symlinks hash their contents, including the tracked `misc/config.toml` link.
- [ ] A current page reuses its stored HTML; a mismatched page rerenders before returning content and stores the current identity.
- [ ] Production app and Nix packaging consume the same identity mechanism, not a repository commit or a store-path hash. FTML's commit-bearing version string must not reintroduce unrelated Git metadata into the key.

## How it works

- [Roster profiling and cold-render finding](../wiki/systems/cobalt-roster-performance.md)
- [Existing data-driven rerender invalidation](cobalt-rerender-invalidation.md)

## Implementation inventory

- `deepwell/render_identity.rs` — canonical length-delimited SHA-256 of logical input paths and contents. Input roots include `src`, `vendor`, `wikidot-forms`, `relation-impl-derive`, `misc`, build/manifests/default-config files, and sibling `locales`; excludes documentation/test/build subtrees.
- `deepwell/build.rs`, `deepwell/Cargo.toml` — fingerprint and Cargo watches; reuse existing SHA-256 dependency during builds.
- `deepwell/src/services/render/service.rs` — required `DEEPWELL_RENDER_ID` as `render-inputs-v1:<hash>`, without a commit-based alternate path.
- `install/dev-deploy.sh` — Git revision remains a deployment label, not a renderer identity override.
- `install/nixos/packages.nix` — keep locales beside the backend in package source so the same build script sees both.

## Tests asserting this spec

- `deepwell/tests/render_identity.rs` — concrete input-copy/mutation, metadata, symlink and missing-input behavior (8/8 at `302c8b907`).
- `deepwell/tests/page_lazy_rerender.rs` — current HTML reuse and stale-build rerender behavior against the database.

## Known gaps (current cycle)

- [ ] Complete build-path, database integration, deployment and independent checks.
- [ ] The initial transition from legacy commit-based keys requires one normal lazy rerender per viewed page. It does not perform a site-wide sweep.

## Out of scope

Image loading/cache policy, gateway capacity, runtime data/schema reconciliation, and a minimal semantic hash of only renderer-reachable Rust. Runtime content/template/include/listing changes retain their existing invalidation responsibilities.
