# Cobalt native package operation

Contract: [native package spec](../../specs/cobalt-native-packages.md). This is package build guidance only. It does not configure a service, create a database or object store, expose a port, add a Cloudflare hostname, migrate data, or deploy to `cobalt-company.sakuin.org`.

## Build

Run from the repository root. Each command realizes one package and creates the usual `result` symlink.

```text
nix build .#deepwell
nix build .#wws
nix build .#framerail
nix build .#silo
```

Remove or inspect `result` between commands; it represents only the most recently realized output. To retain outputs for comparison, supply distinct `--out-link` paths outside tracked source.

```text
nix build .#deepwell --out-link /tmp/cobalt-deepwell
nix build .#wws --out-link /tmp/cobalt-wws
nix build .#framerail --out-link /tmp/cobalt-framerail
nix build .#silo --out-link /tmp/cobalt-silo
```

The packages are pinned to the Sakuin host's nixpkgs revision. Do not use the upstream Docker compose or its deployment scripts for the native Sakuin host.

## Installed outputs

- Deepwell: `bin/deepwell` and `share/deepwell/` containing `config.example.toml`, locales, seeder data, and migrations.
- WWS: `bin/wws`.
- Framerail: `bin/framerail` and `share/framerail/` containing the adapter-node build, runtime dependencies, and `package.json`.
- Silo: `bin/silo`, independently realized at `/nix/store/am512fba05178mdfzlzsngd3anz7w1xb-silo-2026-09-16`.

The Framerail launcher passes `HOST` and `PORT` from its runtime environment to the production adapter-node server. A future service definition must also provide its Deepwell backend settings; do not treat a package realization as a configured server.

## Current proof boundary

At `f737c0c`, Deepwell, WWS, and Framerail realize successfully; exact outputs and command evidence are in `/tmp/claude/cobalt-native-package-build-fixed.log`. `aeb81fe` additionally pins Silo with an isolated Go 1.27.1 toolchain. Agent44 independently realized Silo at `/nix/store/am512fba05178mdfzlzsngd3anz7w1xb-silo-2026-09-16`.

Package realization does not prove a binary starts or that Deepwell, WWS, Framerail, PostgreSQL, Valkey, Silo, the unchanged MinIO client, and routing work together. Runtime module evaluation passes and reports Silo `DEVELOPMENT.GOGET` on Go 1.27.1, but no services have run and no deployment occurred. Client compatibility/security and configured runtime/integration checks remain required before a service/deployment change.
