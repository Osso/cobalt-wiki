# Cobalt security/bug audit — 2026-10-04

Recorded: 2026-10-04. Reconciled against committed fixes through `3a5d8add9` and local proof ledgers. Bounded source audit and local regression evidence, not exhaustive security assurance. **No production deployment of these changes.** Existing deployment claims in other docs concern earlier work.

## Coverage and outcomes

| Boundary | Inspected behavior / outcome | Proof and limits |
|---|---|---|
| Auth / profile | Legacy SSR profile action rejects missing sessions with 401 and MFA-pending (`restricted`) sessions with 403 before saving; unrestricted edits remain available. | `d4f77d16d`; four targeted SSR-action cases RED/GREEN. Not deployed browser/MFA proof. |
| Auth / API / views | Backend `build_request` no longer derives a normal actor from restricted sessions; `view.get_session` excludes restricted sessions. Raw authentication lookup and MFA completion remain available. | `207d38dc1`; actual HTTP login→pending identity denial→MFA completion sequence GREEN after lookup fix `3b1327fbf`. Final combined verification pending after token-consumption changes. See [accounts contract](../../specs/cobalt-accounts.md). |
| Auth / session lookup | Missing `known_user` join caused actual SQL failure during MFA completion; user→known_user→session join fixes lookup. | `3b1327fbf`; actual SQL/MFA failure reproduced, targeted sequence GREEN 1/1. |
| Auth / credential logs | Confirmed credential logging fixed; captured Info diagnostics remain nonempty without tested credentials/tokens. | `77dbc2cfc`; captured-log regression GREEN 1/1 within real login/MFA/session lifecycle. Not a universal logging-safety claim. |
| Auth / authorization tokens | Concurrent reuse allowed both blocked PostgreSQL transactions to authorize with one token; atomic DELETE affected-row check rejects reuse. | RED: two successes instead of one. Fix `4add42a16`; final independent combined GREEN pending. |
| Auth / frontend sign-in | Pending MFA previously returned false success. SSR action now returns explicit 403 with existing form/message, no token/success and no cookie writes. | `bb9192fb0`; actual SSR-action regression GREEN 5/5. Frontend MFA form remains absent; explicit denial is not a bypass. |
| Render / files / media | WWS-wide `Content-Security-Policy: sandbox allow-scripts`, without `allow-same-origin`, isolates documents at router boundary. Supersedes HTML-block-only `4d47d4202` / proof `96efbdcc5`. | `67b401cc0`; actual file/HTML RED exploits then 10 router + 2 Chromium GREEN. PNG fixture correction `1f3551a2b`, spec reconciliation `853a1c119`. See [media contract](../../specs/cobalt-media-routing.md). |
| Persistence / import / API permissions | Reviewed imported-history duplicate handling, filter semantics, historical context, trusted import/rerender RPC reachability and sampled mutation permission boundaries. | Rejected candidates below; no new import fix or import regression proof, no source-site writes, no every-route safety claim. |

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
- Authorization-token concurrency: `cargo test --offline --locked --lib api::tests::security_authorization_token -- --nocapture` reproduced two authorized blocked PostgreSQL transactions instead of one. `4add42a16` checks atomic DELETE affected rows. Relevant change invalidates earlier authorization-consumption GREEN only; final independent verifier owns combined `api::tests::security_` proof, pending at reconciliation time. No inferred final PASS.
- Login: `node --test tests/login.test.ts` at `bb9192fb0`, Vite-loaded actual SSR action, real validation/RPC client and recorded cookie writes: 5/5 GREEN. Accepted login retains both rememberMe options; pending MFA with either option returns 403/form/message without cookie writes, token or success; wrong-credential behavior preserved. Existing missing-pnpm and route-prefix startup diagnostics remain; not a full build gate.

Evidence: `/tmp/cobalt-security-active-file-proof-ledger.md`, `/tmp/cobalt-security-profile-proof-ledger.md`, `/tmp/cobalt-security-login-proof-ledger.md`, `/tmp/cobalt-security-mfa-proof-ledger.md`. Temporary paths are local evidence pointers, not durable tracked artifacts. Backend combined final tests/checks verifier is in progress; no final PASS recorded here. No tests, checks, delegation, push or deployment performed by this documentation update.

## Rejected candidates

- Imported-history same-request duplicates already undergo validation; not an accepted defect.
- OR history filters are intentional; not a filter bug.
- Historical rendering with current context is intentional; not an accepted context leak.
- Trusted loopback import/rerender RPCs are not public exploits absent demonstrated Framerail/gateway exposure. Trust boundary remains relevant; no universal RPC safety claim.

## Remaining gaps / proof limits

- **Renderer HTML stub:** `example.com` output remains an unsupported functional feature, not fixed by WWS isolation.
- **Frontend MFA form:** absent; pending sign-in now fails explicitly with 403 rather than false success. Backend MFA completion proof does not establish an end-to-end frontend MFA flow.
- **Final backend integration:** await independent combined token-consumption/MFA GREEN and final checks. Earlier bounded proofs do not establish final integration PASS.
- **Production / destination isolation:** no production rollout or deployed acceptance proof. S3 destination policy unchanged; upload admission unexamined. HTTP-only avatar/SVG and bounded Chromium fixtures do not establish exhaustive document or application safety.
