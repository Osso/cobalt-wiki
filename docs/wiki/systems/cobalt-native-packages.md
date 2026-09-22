# Cobalt native package operation

Contract: [native package spec](../../specs/cobalt-native-packages.md). This is package build guidance only. It does not configure a service, create a database or object store, expose a port, add a Cloudflare hostname, migrate data, or deploy to `cobalt-company.sakuin.org`.

## Build

Run from the repository root. Each command realizes one package and creates the usual `result` symlink.

```text
nix build .#deepwell
nix build .#wws
nix build .#framerail
```

Remove or inspect `result` between commands; it represents only the most recently realized output. To retain outputs for comparison, supply distinct `--out-link` paths outside tracked source.

```text
nix build .#deepwell --out-link /tmp/cobalt-deepwell
nix build .#wws --out-link /tmp/cobalt-wws
nix build .#framerail --out-link /tmp/cobalt-framerail
```

The packages are pinned to the Sakuin host's nixpkgs revision. Do not use the upstream Docker compose or its deployment scripts for the native Sakuin host.

## Installed outputs

- Deepwell: `bin/deepwell` and `share/deepwell/` containing `config.example.toml`, locales, seeder data, and migrations.
- WWS: `bin/wws`.
- Framerail: `bin/framerail` and `share/framerail/` containing the adapter-node build, runtime dependencies, and `package.json`.

The Framerail launcher passes `HOST` and `PORT` from its runtime environment to the production adapter-node server. A future service definition must also provide its Deepwell backend settings; do not treat a package realization as a configured server.

## Current proof boundary

As of 2026-09-22, only the pnpm fixed-output dependency fetch used to derive the Framerail hash has been proven. It is **not** proof that any package realizes, that a binary starts, or that Deepwell, WWS, Framerail, PostgreSQL, Valkey, S3-compatible storage, and routing work together.

Before a service/deployment change can rely on these outputs, prove all three package realizations; inspect their installed paths; then run configured runtime and integration checks. Deployment remains separate work in the Sakuin NixOS configuration.
