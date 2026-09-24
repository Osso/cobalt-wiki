# Cobalt accounts: set-password links and email

Imported members (see [replica status](../wiki/systems/cobalt-replica-status.md)) start with an unknown random password and a `wikidot-<id>@members.invalid` placeholder email. On migration day each gets their real email and a one-time link to choose a password. Anyone can also ask for a link by email address when they forget their password.

## What it must do

- [x] A set-password link is `/-/set-password/<token>`: 32 random bytes, URL-safe base64 (43 characters). Deepwell stores only the token's SHA-256 (`password_token.token_hash`), never the token.
- [x] A link works once and expires: 7 days for invites, 24 hours for forgotten passwords. Creating a link deletes the user's unused links, so only the newest works.
- [x] Redeeming sets the password through the normal user update (Deepwell's only password rule: not empty) and marks the link used. Unknown or replaced, expired and used links fail with distinct errors that name no user: `PasswordTokenInvalid` 3007, `PasswordTokenExpired` 3008, `PasswordTokenUsed` 3009; empty password `EmptyPassword` 3005.
- [x] Only trusted callers create links for a given user (the invite tool over a loopback Deepwell endpoint, and Deepwell itself when a site admin invites a new member from the [members page](cobalt-member-admin.md)). Framerail never calls `password_token_create` or `password_token_status`.
- [x] A forgotten-password request gets the same reply whether or not the address belongs to an account (case-insensitive match, regular non-deleted users only). The email is sent in the background, so neither timing nor a Mailgun failure reveals the account. A request within 10 minutes of the user's last link sends nothing.
- [x] Email goes through Mailgun's HTTP API (`POST https://api.mailgun.net/v3/{domain}/messages`, basic auth `api:<key>`, form fields `from`, `to`, `subject`, `text`, optional `html`). Only 429 and refused connections are retried (up to 3 attempts, honoring `Retry-After` up to 10 s): sending is not idempotent.
- [x] Without Mailgun configured, every send fails with `EmailSend` (1210, reported as a server error), for every address alike; a partial configuration stops Deepwell at startup. Nothing is ever silently not sent.
- [x] Invites refuse users whose address is not deliverable (the `.invalid` placeholders).
- [x] The invite tool sets each member's email without MailCheck verification (`user_edit` with `bypass_email_verification`, like the import), creates the link and emails it only with `--send`; the default dry run writes nothing and masks addresses. Reruns skip members who chose a password or have a live link (an emailed one when sending) unless their email changed, and at most one member is changed per `--interval` seconds (default 2).
- [x] Pages in the site theme: `/-/set-password/<token>` (new password twice, then a sign-in link), `/-/forgot-password` (email, neutral confirmation), and a "Forgotten your password?" link on the sign-in page.

## Configuration

Runtime environment of Deepwell (all three, or none):

| Variable | Value |
|---|---|
| `MAILGUN_API_KEY` | Mailgun sending key for the domain |
| `MAILGUN_DOMAIN` | `mg.sakuin.org` (US region) |
| `MAILGUN_FROM` | `Cobalt Company <noreply@mg.sakuin.org>` |

Links in emails use the site's preferred domain: `https://<domain>/-/set-password/<token>`.

## RPCs

| Method | Caller | Params | Result |
|---|---|---|---|
| `password_token_create` | trusted only | `user`, `site_id`, `send_email` | `path`, `expires_at`, `emailed` |
| `password_token_status` | trusted only | `user` | `password_chosen`, `pending` (`expires_at`, `emailed`) or null |
| `password_token_redeem` | public (Framerail) | `token`, `password`, `ip_address` | null |
| `password_reset_request` | public (Framerail) | `email`, `site_id` | null |

## Migration day

1. Write the protected map (`chmod 600`), CSV `slug,email` or a JSON object `{"slug": "email"}`.
2. Dry run: `python -m tools.cobalt_migration.member_invites INVITES http://127.0.0.1:27471/jsonrpc SITE_ID PASSWORD_FILE`
3. Send: the same command with `--send`. Without email: `--apply --links-out FILE` writes the links to a new owner-only file.

## Implementation inventory

- `deepwell/migrations/20260924000000_password_token.sql`, `deepwell/src/models/password_token.rs` — the table.
- `deepwell/src/services/password_token.rs`, `deepwell/src/endpoints/password_token.rs`, `deepwell/src/api.rs` — links, redemption, reset requests, emails.
- `deepwell/src/services/email/mailgun.rs`, `deepwell/src/config/secrets.rs` — sender and its startup configuration (`ServerState.mailgun`).
- `deepwell/src/services/user/` — `bypass_email_verification` on `user_edit`.
- `framerail/src/lib/server/load/password.ts`, `framerail/src/lib/server/deepwell/password.ts`, `framerail/src/routes/[x+2d]/set-password/[token]/`, `framerail/src/routes/[x+2d]/forgot-password/`, `framerail/src/lib/page-layout.ts`.
- `tools/cobalt_migration/member_invites.py`.

## Tests asserting this spec

- `deepwell/tests/password_token.rs` (DB-backed, `cobalt_test`): newest link replaces older, hash-only storage, single use, empty password, expired link, unknown link, send needing Mailgun and a deliverable address, email update skipping verification, forgotten password answering alike and emailing only a real account through a fake Mailgun (whose link then works), cooldown, and failing alike without Mailgun. 6/6 passed 2026-09-24 at `bc8d0b4`.
- `deepwell/src/services/email/mailgun.rs` (fake HTTP server): exact form fields and basic auth, no `html` field when absent, 429 retried but 500 not, configuration all-or-none. `deepwell/src/services/password_token.rs`: token format and hashing, undeliverable placeholders. 6/6 passed.
- `framerail/tests/password-pages.test.ts`: redeem RPC params and success page, mismatched/empty passwords, the three link errors, forgot-password RPC params, neutral confirmation, hidden send failure, sign-in link; `framerail/tests/page-layout.test.ts`: site theme. 8/8 passed at `c19564d`.
- `tests/cobalt_migration/test_member_invites.py`: 11/11 passed at `cd18f0f`.
- Not proven: a real Mailgun send, the tool against a running Deepwell with these RPCs, deployment.
