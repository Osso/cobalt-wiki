# Native Wikijump packages

The root flake packages Wikijump for the native NixOS Cobalt deployment. Package definitions live in `install/nixos/packages.nix`; service configuration and deployment are separate work.

## What it must do

- [ ] Pin nixpkgs to the host revision `9b696460ac78b5ccfc17c854d8c976f20456e943`.
- [ ] Build Deepwell and WWS from their checked-in Cargo lockfiles.
- [ ] Install Deepwell configuration examples, locales, seeder data, and migrations under `share/deepwell`.
- [ ] Build Framerail using its frozen pnpm lockfile and a fixed-output dependency hash.
- [ ] Include shared assets during the production SvelteKit build.
- [ ] Run Framerail through adapter-node, retaining runtime `HOST`/`PORT` configuration and production CSRF checks.

## How it works

- [Replica proof boundaries](../wiki/systems/cobalt-replica-status.md): the first full build exposed stable-Rust assertion incompatibility; `f737c0c` fixes that root cause, but the complete retry remains pending.
- [Native package operation](../wiki/systems/cobalt-native-packages.md): build commands, installed output layout, and current proof boundary.
- [Flake package outputs](../../flake.nix): `packages.x86_64-linux.deepwell`, `wws`, and `framerail`.
- [Package definitions](../../install/nixos/packages.nix): Rust packages use `rustPlatform.buildRustPackage`; Framerail uses Node 22, pnpm's Nix hooks, and fetcher version 4.
- Deepwell installs `bin/deepwell` and `share/deepwell/{config.example.toml,locales,seeder,migrations}`. WWS installs `bin/wws`.
- Framerail installs `bin/framerail` and `share/framerail/{build,node_modules,package.json}`. The launcher executes the production server with Nix's Node 22; deployment supplies `HOST`, `PORT`, and backend settings.
- Rust package builds do not run service-dependent tests. The integration owner must run relevant tests against configured PostgreSQL/S3 services.

## Native S3 dependency

Silo replaces the rejected insecure nixpkgs MinIO server; no insecure override is permitted. Pin `pgsty/silo` release `RELEASE.2026-09-16T00-00-00Z` at `2a4d51406b7ed87af5fe6fe0f801f3290f96eb3c` (AGPL-3.0-or-later), containing fixes `efb6e5b00b03e189b44c0349efcee07d440ec428` and `f444b6f37e4494aa043b8786214c7c2c5703e5ff`. Source and vendored dependency hashes are fixed in `packages.nix` and were obtained by targeted Nix fetch/build operations.

Silo requires Go 1.27.1. The host nixpkgs pin provides Go 1.26.3, so packaging uses an isolated Go override with official source SHA-256 `4e408abae126d916b6164627193f2c54f0e3ca1312d693b86db45f862ab238b1`, bootstrapped by Go 1.26.3. Go retains its BSD-3-Clause license. The inherited vendor patch is rebased solely by renaming `loaderstate` to `ld`; its conditional semantics are unchanged. No host toolchain upgrade or runtime toolchain download is introduced.

The module preserves loopback endpoints, storage directories, and runtime credentials. The existing MinIO client remains unchanged; client/server compatibility and security acceptance remain runtime-owner gates, not established by these builds.

Targeted development evidence: Go 1.27.1 builds successfully; source and vendor hashes resolve. Silo compilation reaches dependencies but fails with `no space left on device`. Its executable installation, runtime evaluation, and integration remain unproven. Go's inherited fixup emits `patchelf` diagnostics for static/object files; these were not suppressed. Logs: `/tmp/claude/cobalt-go127-build-rebased.log`, `/tmp/claude/cobalt-silo-vendor.log`, `/tmp/claude/cobalt-silo-build.log`. Final realization and deployment belong to the integration owner.

## Implementation inventory

- `flake.nix`: pinned nixpkgs input, Linux package outputs, formatter.
- `flake.lock`: exact nixpkgs source and NAR hash.
- `install/nixos/packages.nix`: package builds, locked dependency fetching, installed assets and launcher.

## Tests asserting this spec

The fixed pnpm dependency fetch passed. The first full package realization stopped at Deepwell's unstable assertion macros; `f737c0c` replaces them with stable assertions, but all-package realization, runtime integration, and deployment remain unproven.

## Known gaps (current cycle)

- [ ] Retry realization of all three packages after `f737c0c`, then inspect installed outputs.
- [ ] Verify the production Framerail launcher and runtime host/port behavior.
- [ ] Verify the configured service stack and deployment.

## Out of scope

NixOS service units, routing, secrets, data migration, host configuration, deployment, and upstream application changes belong to the integration owner.
