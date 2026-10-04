# Cobalt native package operation

Contract: [native package spec](../../specs/cobalt-native-packages.md). Package builds alone do not configure services or deploy them. Application-only deployment follows the [native runtime contract](../../specs/cobalt-native-runtime.md).

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

## Application-only deployment

Run from the repository root against the existing configured native runtime:

```text
install/dev-deploy.sh all
install/dev-deploy.sh wws
```

`COBALT_HOST` selects the SSH host (default `sakuin`). `all` deploys Deepwell, Framerail, and WWS; `deepwell` and `framerail` select individual applications. Deepwell and Framerail use `/var/lib/cobalt-wiki/app`. WWS builds `.#wws` and copies its Nix closure; `all` completes that preparation before mutating any service. Build or closure-copy failure stops deployment before service mutation.

The script owns the persistent WWS release pin at `/etc/systemd/system.control/cobalt-wiki-wws.service.d/release.conf`. It overrides only `ExecStart`, preserving the full NixOS-managed unit's credentials, environment, user, dependencies, bindings, and resource limits. NixOS's ordinary unit directory is immutable; `system.control` is in the host's persistent unit load path. `/nix/var/nix/gcroots/cobalt-wiki-wws-release` retains the copied package. Each subsequent WWS deployment replaces this pin and GC root. The script reloads systemd, restarts WWS, and restarts/checks the dependent gateway and application services.

This pin is the application release authority, not a competing service definition. Retire it only during an explicit NixOS host rollout whose managed WWS package contains the intended fixes, reconciling removal of the drop-in and GC root, reload, and restart in that rollout. Never remove it alone to reactivate an older insecure package. Application deployment does not update the host flake or perform a NixOS switch.

These commands document script behavior at `9bd9a6a13`, not proof of production deployment. Deployment receipts remain in the [native runtime spec](../../specs/cobalt-native-runtime.md).

## Historical package proof boundary

At `f737c0c`, Deepwell, WWS, and Framerail realize successfully; exact outputs and command evidence are in `/tmp/claude/cobalt-native-package-build-fixed.log`. `aeb81fe` additionally pins Silo with an isolated Go 1.27.1 toolchain. Agent44 independently realized Silo at `/nix/store/am512fba05178mdfzlzsngd3anz7w1xb-silo-2026-09-16`.

Package realization does not prove a binary starts or that Deepwell, WWS, Framerail, PostgreSQL, Valkey, Silo, the unchanged MinIO client, and routing work together. At that package-build checkpoint, runtime module evaluation passed and reported Silo `DEVELOPMENT.GOGET` on Go 1.27.1, but services and deployment were not yet proven. Client compatibility/security and configured runtime/integration checks remain required before a service/deployment change.
