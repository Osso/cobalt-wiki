# Cobalt security/bug audit — 2026-10-04

Recorded: 2026-10-04. Initial repair acceptance through `dd931fa3d`; follow-up repairs and their revision-scoped proof below. Bounded source audit and local regression evidence, not exhaustive security assurance. At that initial inspection, these changes were not deployed: active Deepwell was `fdb02cd205250c0a3c2ac0eada2b9919d194e28d`. Later deployment records below supersede that historical observation.

## Player privacy and Join follow-up — 2026-10-14

verified: 2026-10-14 (bounded evidence; final acceptance pending)

[Player-page and pane policy](../../specs/cobalt-page-read-access.md#player-page-privacy) is the policy SSOT. `80ac39f3a` adds player-category grants in `tools/cobalt_migration/private_categories.sql`; main applied the SQL to production site `6000000`, player category `100000029`, returning `INSERT 4`. Verifier 20 proved 29 anonymous RPC denials, actual regular-member access, and guest media 403. It also found 12 pane actions returning 400 locale errors with metadata diagnostics: that receipt is not a privacy pass.

`641114755` supplies request languages and configured locale to pane authorization; 48 SSR regressions pass. The helper fix is deployed (see [rollout SSOT](cobalt-replica-status.md#player-privacy-and-join-rollout--2026-10-14)), but final verifier 22 remains pending. No post-fix deployed pane-denial acceptance is claimed; main owns the final acceptance receipt.

`678f3c39b` implements response-only Join-module filtering by the actual native `site_member` relation, preserving shared compiled caches. [Member applications](../../specs/cobalt-member-applications.md) owns that contract; deployment does not establish live-browser acceptance.

## Coverage and outcomes

| Boundary | Inspected behavior / outcome | Proof and limits |
|---|---|---|
| Auth / profile | Legacy SSR profile action rejects missing sessions with 401 and MFA-pending (`restricted`) sessions with 403 before saving; unrestricted edits remain available. | `d4f77d16d`; four targeted SSR-action cases RED/GREEN. Not deployed browser/MFA proof. |
| Auth / API / views | Backend `build_request` no longer derives a normal actor from restricted sessions; `view.get_session` excludes restricted sessions. Raw authentication lookup and MFA completion remain available. | `207d38dc1`; actual HTTP login→pending identity denial→MFA completion sequence GREEN after lookup fix `3b1327fbf`. Final combined backend verification passes 2/2 at `aa1fac7b1`; backend files unchanged through `dd931fa3d`. See [accounts contract](../../specs/cobalt-accounts.md). |
| Auth / session lookup | Missing `known_user` join caused actual SQL failure during MFA completion; user→known_user→session join fixes lookup. | `3b1327fbf`; actual SQL/MFA failure reproduced, targeted sequence GREEN 1/1. |
| Auth / credential logs | Confirmed credential logging fixed; captured Info diagnostics remain nonempty without tested credentials/tokens. | `77dbc2cfc`; captured-log regression GREEN 1/1 within real login/MFA/session lifecycle. Not a universal logging-safety claim. |
| Auth / authorization tokens | Concurrent reuse allowed both blocked PostgreSQL transactions to authorize with one token; atomic DELETE affected-row check rejects reuse. | RED: two successes instead of one. Fix `4add42a16`, non-exhaustive result correction `aa1fac7b1`; independent concurrent-consumption case passes within final backend 2/2. |
| Auth / frontend sign-in | Pending MFA previously returned false success. SSR action now returns explicit 403 with existing form/message, no token/success and no cookie writes. | `bb9192fb0`; actual SSR-action regression GREEN 5/5. Frontend MFA form remains absent; explicit denial is not a bypass. |
| Render / files / media | WWS-wide `Content-Security-Policy: sandbox allow-scripts`, without `allow-same-origin`, isolates documents at router boundary. Supersedes HTML-block-only `4d47d4202` / proof `96efbdcc5`. | `67b401cc0`; actual file/HTML RED exploits then 10 router + 2 Chromium GREEN. PNG fixture correction `1f3551a2b`, spec reconciliation `853a1c119`. See [media contract](../../specs/cobalt-media-routing.md). |
| Persistence / import / API permissions | Reviewed imported-history duplicate handling, filter semantics, historical context, trusted import/rerender RPC reachability and sampled mutation permission boundaries. | Rejected candidates below; no new import fix or import regression proof, no source-site writes, no every-route safety claim. |

## Broader API and public-action accounting

The [registered API inventory](cobalt-security-api-inventory.md) enumerates all 182 RPC names/handlers and separates public actions from server wrappers, trusted importer/maintenance callers and internal/unimplemented functions. Registration and caller inventory are complete; method-level authorization, validation and runtime coverage are not exhaustive.

| Boundary | Inspected behavior / disposition | Proof level |
|---|---|---|
| Shared action identity | Raw `session_get` exposed a pending user's ID to vote actions. `78f4b8511` rejects pending sessions; `249dc5ac2` also rejects missing/expired sessions with 401 before returning a non-null action actor. Pending denial remains 403. | Stateful actual SSR vote-action RED/GREEN; eight current cases include missing/expired/pending denial and preserve another user's vote despite forged body IDs. Profile 4/4 retained within current harness 67/67. No deployed vote mutation attempted. |
| Site administration POST | Load-only permission check did not protect POST. `e8f5131da` checks existing Site/Edit policy and trusted routed site before updates. | Actual SSR POST fixture RED then 5/5 GREEN: member, guest, pending, administrator and foreign target. Not actual database/deployed action proof. [Contract](../../specs/cobalt-site-manager.md). |
| Public page-pane reads | Seven legacy actions forwarded raw site/page IDs without the page loader's visibility gate. `7d182daf5` authorizes the exact routed page before reading. | Actual SSR actions/RPC fixtures: 48/48 at `aaded66ca`, including default-page and route-extra semantics. Not database or deployed private-page acceptance. [Contract](../../specs/cobalt-page-read-access.md). |
| Message reply/forward | Inverted recipient predicate rejected recipients and accepted outsiders. `25c536514` repairs it. | Actual registered HTTP RPC/PostgreSQL: 12/12, sender, regular/CC/BCC recipients, outsiders and missing records; denied requests preserve full fixture-owned drafts. Independent backend format/check/readability acceptance retained. [Contract](../../specs/cobalt-message-access.md). |
| Edit error response | Permission error 3106 was reported as 500. `4ffdeaa30` returns 403, retaining other backend errors as 500. | Actual SSR action RED then 2/2 GREEN. Error/status proof only. [Contract](../../specs/cobalt-page-actions.md). |
| Deleted-page lookup | Restore UI really calls `deletedGet`; it must not be removed as unused. `0561568b5` binds request site/slug and reuses actual restore Page/Edit/category/lock checks before returning any candidate metadata. | Registered HTTP/PostgreSQL one comprehensive case at `b3ec1b489`: ten lookups cover actor/site/route/category/lock boundaries and compare full fixture-owned Page/PageRevision models before and after each call. SSR 5/5 at `23e89d483`, retaining 3106→403 status. Independent backend format/check/readability acceptance passes. [Contract](../../specs/cobalt-page-read-access.md#deleted-page-restore-lookup). |
| Gateway/header trust | nginx overwrites ingress site context; Framerail constructs internal RPC context from that site, route and session cookie rather than forwarding client internal headers. | Source trace plus observed loopback listeners: Deepwell 2747, Framerail 3393, WWS 3466, nginx 3088. Direct local callers remain trusted; no external header-forgery experiment. |
| Public user responses | Other-user profiles omit email and self-only fields; own preload/profile/edit responses omit password/MFA secrets; settings discards raw successful edit records. | Source-level field/caller trace. Raw loopback user RPCs still return full models; no public Framerail `user_get` caller found. |

Final follow-up acceptance: frontend `23e89d483` has 0 type errors/warnings, clean scoped lint/format and changed-function readability; backend `b3ec1b489` passes fresh format/check with zero warnings and resolves deleted-helper length/parameter findings. Current targeted SSR harness is 67/67 plus deleted 5/5; registered HTTP/PostgreSQL message 12/12 and deleted one comprehensive ten-lookup test pass. Unchanged auth 2/2, WWS and login evidence is retained, not rerun merely because inventories expanded. Initial 16 actor-type errors, later 14 harness-type errors and the curly warning were corrected, not suppressed; failed stages remain in the receipts. Final receipts: `/tmp/cobalt-security-followup-frontend-acceptance.md`, `/tmp/cobalt-security-final-backend-followup-acceptance.md`. All 182 registration/handler pairs match the tracked inventory without missing/extra entries or duplicate names (`/tmp/cobalt-security-api-inventory-reconciliation.json`). Owned temporary PostgreSQL/Redis stopped; process IDs absent afterward, logs/database artifacts retained (`/tmp/cobalt-security-followup-fixture-shutdown.json`). No push, deployment or production writes.

## Independent follow-up audit (Opus 5.5) — 2026-10-04

The audit above was produced by GPT-6.1-sol. A second, independent pass re-reviewed its repairs and covered the areas it left unexamined. Each fix below has a test that failed before the change; isolated PostgreSQL `cobalt_opus_test` (seeded, `files.json` emptied: no S3) and Redis. **Deployed 2026-10-04** with `install/dev-deploy.sh all` at `63355dfed55f` (now `./deploy.sh all`): exit 0, all four services active. Live checks: `/roster` 200; `/-/html/roster/999` returns 500 instead of a panic; an HTML payload in the block index is returned escaped (`/tmp/claude/cobalt-deploy-security.log`). Existing compiled pages pick up the gallery fix only on rerender.

| Fix | Defect | Proof |
|---|---|---|
| `32f6f1824` | Any signed-in user could vote on any page (other sites, private) with any `i16` value; Sol's vote test passed without its fix. | `deepwell/tests/page_vote.rs`, `framerail/tests/actor-session.test.ts` 12/12 |
| `1fe2fb050` | `[[gallery]]` list lines with `://` bypassed the URL scheme allow-list: stored `javascript:` link on the main origin. | ftml `gallery` tests 5/5. Existing compiled HTML changes only on rerender. |
| `0da6a51fd` | `/-/user` changed email without the current password (settings requires it): stolen session → reset takeover. | `user-edit.test.ts` 5/5 |
| `14cb27dfa` | Renaming onto another user's past name took it over without a rename token. | `deepwell/tests/user_rename.rs` |
| `f1f8ab097`, `1e3444394` | Password reset and settings password change left other sessions valid. | `password_token.rs` 7/7, `settings.test.ts` 24/24 |
| `93a8d30ab` | Basic error pages interpolated URL values (page slug, filename, block index, hash) into HTML unescaped. | `deepwell/tests/basic_error.rs` |
| `16f522cfc` | WWS panicked on S3 non-200 (`/-/html/<page>/<missing index>`, missing avatar blob). | WWS handler tests 15/15 |
| `2e5416146` | `session_renew` upgraded MFA-pending sessions; deleted users could log in and kept sessions. | `deepwell/tests/auth_lifecycle.rs` |
| `5a59d5489` | Form actions returned 500 for permission, auth, conflict and input errors. One code-range mapping. | 23 affected action test files: 214/215, the failure is pre-existing (below). |
| `c9929adeb` | Native history "before" paging returned the oldest revisions; unknown hidden-field names were stored and panicked reads; revision update ignored site/page and swallowed its error. | `deepwell/tests/page_revision_range.rs` |
| `4b66ececc` | Two concurrent MFA logins could spend one recovery code (list rewritten from a stale snapshot). Atomic conditional removal. | `api::tests::security_recovery_code_has_only_one_concurrent_use`; security tests 3/3 |
| `0958737da` | File history "before" paging had the same oldest-first bug. | `deepwell/tests/file_revision_range.rs` |
| `81c148b48` | File revision edits changed any revision ID regardless of file/page/site. Page ownership test now uses a real second page and fails with the check removed. | `file_revision_range.rs`, `page_revision_range.rs` |

Final gates on the changed code: `cargo fmt --check` (deepwell, wws, ftml), `cargo check --all-targets` 0 warnings (deepwell, wws), svelte-check 0/0, scoped ESLint/Prettier pass.

Independent verifier: PASS for `32f6f1824`–`c9929adeb` (targeted reruns; no regression found). Its findings led to `0958737da` and `81c148b48`. `file_revision_permission` tests need S3 and were not run here. Status-code change: login wrong password is now 401, conflicts 409, input errors 400.

**Re-review of Sol's repairs:** sound, except the vote boundary above. The WWS-wide `sandbox` CSP may stop Chromium from displaying PDFs (not tested). The deleted-page lookup denies the whole lookup when any one deleted copy is locked.

**Open, not fixed:**
- TOTP codes can be replayed within the skew window; the fix needs a stored last-used time step (schema change). MFA has no frontend.
- Revision rows already stored with unknown hidden-field names still panic on read; writes now reject them.
- The settings password change ends other sessions in a second call after saving; if that call fails, the password is changed but other sessions remain.
- Email is not unique (no constraint; update path unchecked; case handling differs between login and reset). Fixing it needs a data audit and a migration decision.
- Re-uploading a hard-deleted blob stores the S3 object again before the blacklist check; the avatar cache is not cleared on hard delete. Needs S3 to test.
- File restore skips the name-conflict check and drops the rename/move (fix needs S3-backed file fixtures to test). Concurrent same-name uploads create duplicates. WWS caches slug→page ID with no TTL.
- The gateway does not clear client `X-Wikijump-*` internal headers. The effect is limited to the requester's own response, and error output is now escaped.
- Pre-existing test failure: `framerail/tests/page-block.test.ts` "starts disabled" expects attribute order that Svelte does not produce.
- Upstream `install/dev` Docker stack publishes Postgres, Valkey (no auth) and Deepwell on all interfaces; this does not affect the NixOS deployment.

## Local proof

### WWS document isolation

- Router RED: actual public file response returned 200 `text/html`, exact active payload, cache and ETag, without CSP. Chromium read parent secret, cookie and private API response with default and explicit credentials. Actual HTML-block/file payload captures establish the exploit boundary, not upload-admission coverage.
- Router GREEN at `67b401cc0`: `cargo test handler:: -- --ignored --nocapture` in `wws`, isolated Redis and HTTP fixtures; 10/10 pass. Shared CSP covers public HTML/SVG, PNG/font/text, downloads, avatar metadata-selected HTML and code/text blocks. Bytes, MIME, disposition, access-denial ordering, cache behavior and private presigned redirects preserved.
- Chromium GREEN at `67b401cc0`: `node --test tests/html-block-isolation.test.mjs` in `framerail`, actual file and HTML-block router captures; 2/2 pass, iframe and direct navigation. Scripts execute; parent/cookie reads throw SecurityError; API/credentialed API reads fail with TypeError; message origin is `null`; direct-navigation cookie read throws SecurityError.
- Corrected PNG fixture byte/disposition regression at `1f3551a2b`: 1/1 GREEN. Other router/browser evidence unchanged. PNG proof asserts valid fixture bytes, not UI rendering. Avatar/SVG have actual HTTP-contract proof only, not separate Chromium execution.
- Two outbound requests still observed per iframe. No outgoing-request/CSRF-prevention claim; sandbox does not replace API CORS or CSRF protections. Main Framerail documents unaffected. Redirect destinations require their own isolation; **S3 destination policy unchanged**. Upload admission and production runtime not examined or changed. Temporary Redis instances stopped.

### Authentication and logging

- Profile: `node --test tests/user-edit.test.ts` in `framerail`, four RED/GREEN cases: restricted, missing-cookie, null-session and unrestricted saves. Denied cases preserve saved profile state. Owned-file hashes establish GREEN while HEAD was `96efbdcc5`; action/test changes subsequently committed as `d4f77d16d`.
- Backend MFA: targeted actual HTTP password login first reproduced restricted session with non-null request actor. After `207d38dc1`, pending actor/viewer denial passed but MFA completion failed. Diagnostic revisions `f0f5517da` / `648d64384` established missing FROM-clause entry for `known_user`; `3b1327fbf` fixed the join. `cargo test --offline --locked --lib api::tests::mfa_pending_session -- --nocapture` then GREEN 1/1: login→pending identity denial→correct recovery code→unrestricted actor/profile→pending token consumed.
- Credential logs at `77dbc2cfc`: same real sequence, captured Info-log regression GREEN 1/1. Nonempty diagnostics cover login, MFA, session renewal/invalidation and issued/consumed authorization token; exclude both issued session tokens, authorization token, fixture password and recovery code.
- Authorization-token concurrency: `cargo test --offline --locked --lib api::tests::security_authorization_token -- --nocapture` reproduced two authorized blocked PostgreSQL transactions instead of one. `4add42a16` checks atomic DELETE affected rows. Relevant change invalidates earlier authorization-consumption GREEN only; final independent `api::tests::security_` proof passes 2/2 at `aa1fac7b1`, with zero Rust warnings.
- Login: `node --test tests/login.test.ts` at `bb9192fb0`, Vite-loaded actual SSR action, real validation/RPC client and recorded cookie writes: 5/5 GREEN. Accepted login retains both rememberMe options; pending MFA with either option returns 403/form/message without cookie writes, token or success; wrong-credential behavior preserved. Existing missing-pnpm and route-prefix startup diagnostics remain; not a full build gate.

Evidence: `/tmp/cobalt-security-active-file-proof-ledger.md`, `/tmp/cobalt-security-profile-proof-ledger.md`, `/tmp/cobalt-security-login-proof-ledger.md`, `/tmp/cobalt-security-mfa-proof-ledger.md`. Temporary paths are local evidence pointers, not durable tracked artifacts. Independent acceptance: backend 2/2, retained WWS router 10/10 (corrected PNG case rechecked), profile 4/4 and login 5/5; Chromium 2/2 re-proven at `dd931fa3d`. Both Rust crates pass `cargo fmt --check` and `cargo check --offline --locked`; frontend typecheck reports 0 errors/0 warnings, scoped ESLint/Prettier pass. Harness readability passes 12/12 functions, maximum cognitive complexity 10, after named response/iframe/direct-navigation extraction. Retained source/config hashes match 54/54 comparisons; unchanged suites were not rerun. Final ledgers: `/tmp/cobalt-security-final-proof-ledger.md` and `/tmp/cobalt-security-final-harness-acceptance.md`. Initial compiler/type/readability failures and their scoped corrections remain recorded in the ledgers.

## Rejected candidates

- Imported-history same-request duplicates already undergo validation; not an accepted defect.
- OR history filters are intentional; not a filter bug.
- Historical rendering with current context is intentional; not an accepted context leak.
- Trusted loopback import/rerender, blob upload/cancel, generic role/domain/user and direct getter RPCs are caller-trusting internal interfaces, not public exploits merely because they accept payload actor IDs. Actual listeners are loopback-bound; public wrappers/actions must supply and constrain authority. No universal RPC safety claim.
- Settings' lack of a local pending-session check is not a bypass: its own `user_view` lookup already excludes restricted viewers, while watching derives the backend filtered actor.
- A normal voter cannot select another actor through inspected public actions: they ignore body user IDs and use the cookie session's ID. Stateful SSR cast/cancel fixtures preserve the other user's vote. Direct trusted RPC actor substitution is a different boundary.
- Parent names, voter identities and historical file metadata on a visible page do not establish a new confidentiality policy. File-info hidden-comment omission is explicitly presentation-only in its existing contract; no speculative API redaction added.
- Authorization-token single-consumption proof covers the verifier, not a claim that ordinary registration/site creation consumes a token. Only bot creation was found to call it in production code; public regular registration is intentionally present.

## Remaining gaps / proof limits

- **Renderer HTML stub:** `example.com` output remains an unsupported functional feature, not fixed by WWS isolation.
- **Frontend MFA form:** absent, with no frontend setup/enable/verify route or existing supported completion contract found. Pending sign-in fails explicitly with 403 instead of false success. Backend MFA completion does not establish an end-to-end frontend flow; adding one would be a separate feature, not this audit's repair.
- **Environment diagnostics:** Vite's optional pnpm-version probe still emits `pnpm: command not found`; required type/lint gates pass. Earlier unrelated route-prefix startup diagnostics remain in retained SSR logs. No warning suppression or package installation.
- **Production / destination isolation:** no production rollout or deployed acceptance proof. S3 destination policy unchanged; upload admission unexamined. HTTP-only avatar/SVG and bounded Chromium fixtures do not establish exhaustive document or application safety.
