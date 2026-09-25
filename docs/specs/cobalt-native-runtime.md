# Cobalt native runtime

`install/nixos/module.nix` defines an isolated, loopback-only Wikijump runtime using the [native packages](cobalt-native-packages.md). It does not expose a website or change Sakuin services. Root `deploy.sh` updates only the existing local preview Deepwell user service; `install/dev-deploy.sh` targets production Sakuin. See the [runtime integration guide](../wiki/systems/cobalt-native-runtime.md) for deployment integration; that guide remains main-owned.

Local preview deployment requires SQLx CLI on PATH (for example, `nix shell nixpkgs#sqlx-cli --command ./deploy.sh`). Root `deploy.sh` validates the protected local `environment.json` DATABASE_URL targets `cobalt_local_full` on loopback port 25432, confirms the preview unit is active, builds Deepwell, runs `sqlx migrate run` against that database from `deepwell/`, then restarts the unit only after migration success. This workflow does not deploy production Sakuin.

## What it must do

- [ ] Run PostgreSQL 17, Valkey, S3 storage, Deepwell, production Framerail, and WWS as namespaced native systemd units, without Docker.
- [ ] Keep all TCP listeners on loopback; PostgreSQL accepts only local Unix-socket peer authentication.
- [ ] Limit the complete wiki service group to 2 GiB through `cobalt-wiki.slice` without modifying existing services or firewall policy.
- [ ] Keep database, object, cache, and provisioning state in dedicated owner-only directories under `/var/lib/cobalt-wiki*`.
- [ ] Load credentials from a required runtime EnvironmentFile outside the Nix store; storage and backend use the same S3 credentials without secret command-line arguments.
- [ ] Create separate `cobalt-wiki-files` and `cobalt-wiki-text-blocks` buckets without public bucket policies.
- [ ] Disable seeding during ordinary startup and refuse backend startup before explicit production provisioning.
- [x] Offer a manual-only bootstrap unit when a private reviewed seed directory is explicitly supplied; apply packaged migrations, seed, and mark success only after both commands succeed.
- [x] Use a fixed HTTPS Framerail origin and production adapter-node server, without disabling CSRF checks. Framerail permits cross-origin form POSTs only when `FRAMERAIL_ENV=local`; production and unset environments retain same-origin enforcement.
- [x] Keep dependent WWS/Framerail startup ordered after a successful Deepwell JSON-RPC ping, with bounded retries for transient startup failures. Four HTTP tests pass; production switch completed with successful first WWS ping and zero automatic restarts.

## How it works

- [Native package operation](../wiki/systems/cobalt-native-packages.md)
- [Runtime integration](../wiki/systems/cobalt-native-runtime.md) — integration guide pending.

## Implementation inventory

- `install/nixos/module.nix` — `services.cobaltWiki` options, runtime TOML, initialization helpers, private service units, and resource slice.
- `install/nixos/packages.nix` — supplies Deepwell, WWS, Framerail, and the pinned Silo storage package.
- `install/nixos/wait_deepwell.py` — bounded HTTP readiness check used by Deepwell's `ExecStartPost` before ordered dependents start.
- The existing local preview runtime at `/home/osso/.local/share/cobalt-wiki/local-full/deepwell.toml` uses two workers and a one-to-two-second empty-queue poll for template-refresh proof. This local-only setting does not change the module or production configuration.

## Tests asserting this spec

`framerail/tests/csrf-config.test.mjs` uses an isolated SvelteKit fixture to assert same-origin and cross-origin form POST behavior for local, production, and unset environments; it does not verify deployment. Final gate `/tmp/cobalt-csrf-final-gate.json` passes at `5c6f1fba4`: four retained tests cover six real Kit POST cases (both local origins `200`; production/unset same-origin `200`, cross-origin `403`), direct `svelte-check` reports 0 errors and 0 warnings, scoped ESLint covers the test, Prettier covers config and test, and readability passes. Existing ESLint policy intentionally ignores `svelte.config.js`; the behavior is validated through Kit, not represented as ESLint coverage for that ignored file. `tests/cobalt_migration/test_readiness.py` exercises refused connections before listener activation, transient HTTP failures, Retry-After timing, invalid RPC responses and bounded exhaustion. Production switch verification remains necessary to prove systemd ordering. Targeted module evaluation establishes generated configuration, not running-service or migration correctness. A separate isolated local PostgreSQL 17, Valkey, and Silo environment completed migrations and the stock development seeder, then supported three Deepwell form-edit DB tests and four view-helper tests; its protected logs are under `/home/osso/.local/share/cobalt-wiki/integration/`. That proves only those local application paths, not this NixOS module's services, production provisioning, private ACLs, or deployment.

## Known gaps (current cycle)

- [x] `aeb81fe` replaces rejected insecure `pkgs.minio` with pinned `pgsty/silo` release `RELEASE.2026-09-16T00-00-00Z` (`2a4d51406b7ed87af5fe6fe0f801f3290f96eb3c`), which contains fixes for CVE-2026-40344 and CVE-2026-41145. Agent44 independently realized Silo at `/nix/store/am512fba05178mdfzlzsngd3anz7w1xb-silo-2026-09-16`; enabled-module evaluation passes and reports `DEVELOPMENT.GOGET` on Go 1.27.1. Do not permit the rejected MinIO package as a fallback.
- [x] Sakuin's initial native switch completed on 2026-09-22 as closure `bna501zq169hkxrkazv3b4j5qcfby1fn`; the existing Sakuin readiness check passed.
- [x] Production bootstrap completed on 2026-09-22 after seed correction `660fe2c`; Deepwell assigned Cobalt Company site ID `6000000`.
- [ ] Verify module-managed service restart persistence, actual socket bindings, capped-slice behavior under load, and continued Sakuin health after subsequent Cobalt activation.
- [ ] Email-provider configuration and credentials must be supplied deliberately; this module does not select a mock provider.
- [x] The optional [POC gateway](cobalt-poc-gateway.md) is publicly reachable through the existing Sakuin tunnel using site ID `6000000`. Public tests verify authentication/no-index headers and archive-matching image downloads; source ACL parity remains open. See [current proof](../wiki/systems/cobalt-replica-status.md).

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
