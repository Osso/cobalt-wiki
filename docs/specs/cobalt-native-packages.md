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

- [Native package operation](../wiki/systems/cobalt-native-packages.md): build commands, installed output layout, and current proof boundary.
- [Flake package outputs](../../flake.nix): `packages.x86_64-linux.deepwell`, `wws`, and `framerail`.
- [Package definitions](../../install/nixos/packages.nix): Rust packages use `rustPlatform.buildRustPackage`; Framerail uses Node 22, pnpm's Nix hooks, and fetcher version 4.
- Deepwell installs `bin/deepwell` and `share/deepwell/{config.example.toml,locales,seeder,migrations}`. WWS installs `bin/wws`.
- Framerail installs `bin/framerail` and `share/framerail/{build,node_modules,package.json}`. The launcher executes the production server with Nix's Node 22; deployment supplies `HOST`, `PORT`, and backend settings.
- Rust package builds do not run service-dependent tests. The integration owner must run relevant tests against configured PostgreSQL/S3 services.

## Implementation inventory

- `flake.nix`: pinned nixpkgs input, Linux package outputs, formatter.
- `flake.lock`: exact nixpkgs source and NAR hash.
- `install/nixos/packages.nix`: package builds, locked dependency fetching, installed assets and launcher.

## Tests asserting this spec

Full package realization and runtime integration remain unproven. The pnpm dependency fetch was run to obtain its fixed hash; this is not proof of a successful Framerail build.

## Known gaps (current cycle)

- [ ] Realize all three packages and inspect installed outputs.
- [ ] Verify the production Framerail launcher and runtime host/port behavior.
- [ ] Verify Rust compatibility with the pinned host toolchain.

## Out of scope

NixOS service units, routing, secrets, data migration, host configuration, deployment, and upstream application changes belong to the integration owner.
