# Cobalt compiled-render identity

Stored HTML is reused while its rendering-input identity remains current. `deepwell/render_identity.rs` supplies the content fingerprint used by the Cargo build script; local Cargo, app deployment and Nix packages use that same calculation. This addresses the first-view build-mismatch cost measured in the [roster profile](../wiki/systems/cobalt-roster-performance.md).

## What it must do

- [x] Equal input contents produce equal identities regardless of checkout path, traversal order, filesystem timestamps or Git metadata.
- [x] Documentation, frontend, tests and build-output changes do not change the identity.
- [x] Backend Rust, vendored dependencies, manifests/lockfile, default configuration, embedded templates and locale changes change the identity. The hash is deliberately conservative: unrelated backend API code can still invalidate HTML; it does not attempt semantic dependency analysis.
- [x] Adding/removing input files changes identity; missing required inputs fail with the affected path. File symlinks hash their contents, including the tracked `misc/config.toml` link.
- [x] A current page reuses its stored HTML; a mismatched page rerenders before returning content and stores the current identity.
- [x] Production app and Nix packaging consume the same identity mechanism, not a repository commit or a store-path hash. FTML's commit-bearing version string must not reintroduce unrelated Git metadata into the key.

## How it works

- [Roster profiling and cold-render finding](../wiki/systems/cobalt-roster-performance.md)
- [Existing data-driven rerender invalidation](cobalt-rerender-invalidation.md)

## Implementation inventory

- `deepwell/render_identity.rs` — canonical length-delimited SHA-256 of logical input paths and contents. Input roots include `src`, `vendor`, `wikidot-forms`, `relation-impl-derive`, `misc`, build/manifests/default-config files, and sibling `locales`; excludes documentation/test/build subtrees.
- `deepwell/build.rs`, `deepwell/Cargo.toml` — fingerprint and Cargo watches; reuse existing SHA-256 dependency during builds.
- `deepwell/src/services/render/service.rs` — required `DEEPWELL_RENDER_ID` as `render-inputs-v1:<hash>`, without a commit-based alternate path.
- `install/dev-deploy.sh` — Git revision remains a deployment label, not a renderer identity override.
- `install/nixos/packages.nix` — keep locales beside the backend in package source so the same build script sees both.
- `install/{local,dev,prod}/deepwell/Dockerfile`, `install/local/docker-compose.dev.yaml` — provide the same sibling locale input and mounted helper in container builds; generated dependency copies live under `.cargo/vendor`, outside authored render inputs.

## Tests asserting this spec

- `deepwell/tests/render_identity.rs` — concrete input-copy/mutation, metadata, symlink and missing-input behavior (8/8 at `302c8b907`).
- `deepwell/tests/page_lazy_rerender.rs` — current HTML reuse and stale-build rerender behavior against the database (2/2, `/tmp/cobalt-render-identity-page-tests.log`).
- Full Nix package build passed at `60d7abaf6`: `/nix/store/da6ag74zdv7m3pvsg92s3ygi763rfzr2-cobalt-deepwell-2026.8.20`; receipt `/tmp/cobalt-render-identity-nix-build2.log`.

## Initial deployment proof

Local and production backend deployment passed at `60d7abaf6`. The race-roster's first ordinary view transitioned its legacy key to `render-inputs-v1:5eb243ea930e3b22a5bed3d752a78f342a1bbbcb84caa16e7c43cdbb1a2b814f` in 527 ms; the next host-local RPC took 13 ms. Revision `3000002431` and body hash remained unchanged. Receipts: `/tmp/cobalt-render-identity-before-transition.csv`, `/tmp/cobalt-render-identity-before-doc-deploy.csv`, `/tmp/cobalt-render-identity-transition-{0,1}.log`.

## Verified redeployment behavior

A documentation-only commit (`30f6d1b82`) was deployed after the initial rollout. The first subsequent race-roster RPC took 23 ms. Its revision ID, revision `updated_at`, rendering identity and compiled body hash exactly matched the pre-deploy row: no rerender occurred. Local debug and production deploy builds report the same content identity. Receipts: `/tmp/cobalt-render-identity-{before-doc-deploy,after-doc-deploy}.csv`, `/tmp/cobalt-render-identity-after-doc-deploy-timing.log`, `/tmp/cobalt-render-identity-local-production.json`.

Independent verification passed Cargo formatting/checking, retained the 8 fingerprint and 2 database tests, checked the Nix package/input identity, and matched the running production binary to the deploy artifact (`/tmp/cobalt-render-identity-verifier.md`). Container Dockerfile checks and Compose validation passed separately (`/tmp/cobalt-render-identity-container-gate.txt`); full container images were not compiled. Native app and Nix builds were compiled.

## Known gaps (current cycle)

No blocker for the deployed native path. The first transition from legacy commit-based keys still requires one normal lazy rerender per viewed page; the canary verified that transition. No site-wide sweep was performed. Backend runtime-input changes intentionally still invalidate compiled HTML.

## Out of scope

Image loading/cache policy, gateway capacity, runtime data/schema reconciliation, and a minimal semantic hash of only renderer-reachable Rust. Runtime content/template/include/listing changes retain their existing invalidation responsibilities.
