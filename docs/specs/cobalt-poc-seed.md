# Cobalt POC seed generation

`tools/cobalt_migration/poc_seed.py` generates private, content-free input for Deepwell's existing seeder. It does not seed a database. Runtime provisioning belongs to [the native runtime](cobalt-native-runtime.md); imported source attribution remains separate from the technical account.

## What it must do

- [x] Require explicit output directory, HTTPS origin, site slug, default page, and administrator password file; never guess the front page.
- [x] Emit one Cobalt Company site using native subdomain routing, the supplied default page, Wikidot layout, English locale, and source CC-BY-SA 3.0 license. Register no custom domains or preferred-domain override. Emit no demo sites, pages, attachments, or filters.
- [x] Preserve system IDs −2 through −5 as password-disabled system records; create only one login-enabled account, `cobalt-import` (ID −1), explicitly described as a technical administrator/import principal, not a source author.
- [x] Consume an owner-owned regular 0600 password file without following symlinks. Accept one trailing newline; reject control characters, stock demo credentials, and secrets shorter than 16 characters. Never print the secret.
- [x] Write exactly six JSON files in a new 0700 directory, each 0600. Refuse existing directories, files, and symlinks without modifying them.
- [x] Preserve stock role permissions and the parent relationships actually recognized by the seeder, using its kebab-case field names. Do not introduce role inheritance by interpreting ignored stock keys.

## How it works

- Seeder input contract: `deepwell/src/database/seeder/data.rs` (`SeedData::load`, `User`, `Site`, `Role`). Only `users`, `sites`, `pages`, `files`, `filters`, and `roles` JSON are read; no seed TOML or `permissions.json` is required.
- Native hostname is the site slug plus the runtime main domain. Emit `domains: []` and omit `preferred-domain`; registering the native hostname as a custom domain is rejected. The caller must ensure the supplied origin matches that runtime hostname; the origin supplies the technical account's email domain.
- Seeder behavior: `deepwell/src/database/seeder/mod.rs` grants the `admin` role to ID −1 and skips an already-seeded database when that user exists. The generator neither creates a provisioning marker nor decides whether database seeding should run.
- Stock `parent_role` and `is-system` fields are not consumed by the current kebab-case `Role` parser. Generated `parent-role` preserves the currently parsed value (null), rather than silently enabling a different hierarchy.
- Stock role definitions are POC defaults, **not imported source ACLs**. The all-route authentication gate remains mandatory. No registration-disable option was found in the inspected runtime configuration; generation does not claim to disable registration.
- Retained local authenticated HTML links Home to `/home:start`; the acquired canonical listing contains `home:start`, not `start`. This supports the caller choosing `--default-page home:start`; the generator still requires the argument and does not inspect private exports.

## Implementation inventory

- `tools/cobalt_migration/poc_seed.py` — validates inputs and writes protected seed JSON using tracked system users/roles.
- `deepwell/seeder/users.json`, `deepwell/seeder/roles.json` — read-only source of required system records and current role definitions; stock regular-user passwords are never copied.

Invocation from the repository root:

```text
python -B -m tools.cobalt_migration.poc_seed --output /private/new-seed --admin-password-file /private/admin-password --origin https://cobalt-company.sakuin.org --slug cobalt-company --default-page home:start
```

The caller supplies a randomly generated administrator secret separately from the reverse-proxy access credential. The output's `users.json` necessarily contains that plaintext secret for the existing seeder and must remain outside Git.

## Tests asserting this spec

- `tests/cobalt_migration/test_poc_seed.py` — emitted JSON contracts, system/admin identities, role permissions, private modes, existing-output refusal, credential rejection, and CLI behavior.

## Known gaps (current cycle)

- [x] Production bootstrap completed on 2026-09-22 using corrected seed generator `660fe2c`; Deepwell assigned site ID `6000000`.
- [ ] Independently verify generated admin login, assigned role, and absence of demo content after bootstrap.

## Out of scope

Database writes, network access, deployment, source import, source authorship claims, source ACL migration, credential generation, and reverse-proxy configuration. Main provisions the seed and obtains the assigned site ID.
