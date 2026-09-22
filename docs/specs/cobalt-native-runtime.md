# Cobalt native runtime

`install/nixos/module.nix` defines an isolated, loopback-only Wikijump runtime using the [native packages](cobalt-native-packages.md). It does not expose a website or change Sakuin services. See the [runtime integration guide](../wiki/systems/cobalt-native-runtime.md) for deployment integration; that guide remains main-owned.

## What it must do

- [ ] Run PostgreSQL 17, Valkey, S3 storage, Deepwell, production Framerail, and WWS as namespaced native systemd units, without Docker.
- [ ] Keep all TCP listeners on loopback; PostgreSQL accepts only local Unix-socket peer authentication.
- [ ] Limit the complete wiki service group to 2 GiB through `cobalt-wiki.slice` without modifying existing services or firewall policy.
- [ ] Keep database, object, cache, and provisioning state in dedicated owner-only directories under `/var/lib/cobalt-wiki*`.
- [ ] Load credentials from a required runtime EnvironmentFile outside the Nix store; storage and backend use the same S3 credentials without secret command-line arguments.
- [ ] Create separate `cobalt-wiki-files` and `cobalt-wiki-text-blocks` buckets without public bucket policies.
- [ ] Disable seeding during ordinary startup and refuse backend startup before explicit production provisioning.
- [ ] Offer a manual-only bootstrap unit when a private reviewed seed directory is explicitly supplied; apply packaged migrations, seed, and mark success only after both commands succeed.
- [ ] Use a fixed HTTPS Framerail origin and production adapter-node server, without disabling CSRF checks.

## How it works

- [Native package operation](../wiki/systems/cobalt-native-packages.md)
- [Runtime integration](../wiki/systems/cobalt-native-runtime.md) — integration guide pending.

## Implementation inventory

- `install/nixos/module.nix` — `services.cobaltWiki` options, runtime TOML, initialization helpers, private service units, and resource slice.
- `install/nixos/packages.nix` — supplies Deepwell, WWS, Framerail, and the pinned Silo storage package.

## Tests asserting this spec

No checked-in runtime tests yet. Targeted module evaluation can establish generated configuration, not running-service or migration correctness. Requirements remain unchecked until behavioral runtime proof exists.

## Known gaps (current cycle)

- [x] `aeb81fe` replaces rejected insecure `pkgs.minio` with pinned `pgsty/silo` release `RELEASE.2026-09-16T00-00-00Z` (`2a4d51406b7ed87af5fe6fe0f801f3290f96eb3c`), which contains fixes for CVE-2026-40344 and CVE-2026-41145. Agent44 independently realized Silo at `/nix/store/am512fba05178mdfzlzsngd3anz7w1xb-silo-2026-09-16`; enabled-module evaluation passes and reports `DEVELOPMENT.GOGET` on Go 1.27.1. Do not permit the rejected MinIO package as a fallback.
- [ ] Main must import the module and supply domains, the private EnvironmentFile, and reviewed production provisioning data.
- [ ] No runtime services have run. Verify PostgreSQL initialization, SQL migrations, storage buckets, application startup, restart persistence, and actual socket bindings.
- [ ] Main must verify all services remain in the capped slice under load and that existing Sakuin services remain healthy.
- [ ] Email-provider configuration and credentials must be supplied deliberately; this module does not select a mock provider.
- [ ] Proxy routing must preserve the public HTTPS origin and supply trusted Wikijump site headers; no reverse proxy is provided here.

### Integration contract

Required options are `enable`, `environmentFile`, `mainDomain`, and `filesDomain`. `packages` defaults to the sibling package definitions and can be supplied by the host's pinned package set. `bootstrapSeedDirectory` defaults to null.

The EnvironmentFile supplies `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY`, and the chosen email provider's variables. It must not override module-owned bind addresses, database URL, or bucket configuration. Silo's MinIO-compatible root credentials are derived from those S3 credentials in the service process, not serialized into the Nix store. The unchanged MinIO client has not yet been accepted for compatibility/security with Silo.

With a non-null private seed directory, main may explicitly start `cobalt-wiki-bootstrap.service`; it is never enabled at boot or pulled in by another unit. The unit runs SQLx migrations and Deepwell's supported `DEEPWELL_RUNTIME_ACTION=run-seeder` operation. It refuses to run when `/var/lib/cobalt-wiki/provisioned` already exists. Ordinary Deepwell always has `run-seeder=false`. Alternatively, main may provision externally and create that marker after successful provisioning. Seed files must be readable by the `cobalt-wiki` Unix account and must not contain upstream demo accounts, passwords, or sites.

Units are `cobalt-wiki-{postgresql,cache,storage,buckets,deepwell,framerail,wws}.service`, plus the optional manual bootstrap unit. PostgreSQL uses `/run/cobalt-wiki-postgresql`, database `cobalt_wiki`, and peer role `cobalt-wiki`. The dedicated cluster's bootstrap superuser is that same dedicated Unix account. State directories are `cobalt-wiki`, `cobalt-wiki-postgresql`, `cobalt-wiki-cache`, `cobalt-wiki-s3`, and `cobalt-wiki-s3-config` under `/var/lib`; the bucket initializer has private runtime state under `/run/cobalt-wiki-buckets`.

Valkey binds `127.0.0.1:6381`; S3/console bind `127.0.0.1:9000/9001`; Deepwell, Framerail, and WWS bind `127.0.0.1:2747/3393/3466`. The generated Deepwell TOML retains the upstream non-secret configuration structure, overrides deployment paths/domains/bind address, and never selects the bundled demo seed directory.

## Out of scope

- Host flake integration, deployment scripts, DNS, Cloudflare, Caddy, public listeners, and firewall changes are main-owned.
- Production secrets, seed data, data migration, backups, and application behavior changes are separate work.
- This module does not provision or mutate an existing PostgreSQL, Redis, or MinIO instance.
