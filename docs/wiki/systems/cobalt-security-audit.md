# Cobalt security/bug audit — 2026-10-04

Recorded: 2026-10-04. Bounded source audit and local regression evidence, not exhaustive security assurance. **No production deployment of these changes.** Existing deployment claims in other docs concern earlier work.

## Coverage and outcomes

| Boundary | Inspected behavior / outcome | Proof and limits |
|---|---|---|
| Auth / profile | Legacy SSR profile action now rejects missing sessions with 401 and MFA-pending (`restricted`) sessions with 403 before saving; unrestricted edits remain available. | `d4f77d16d`; four targeted SSR-action cases RED/GREEN. Not deployed browser/MFA proof. |
| Auth / API / views | Backend `build_request` no longer derives a normal actor from restricted sessions; `view.get_session` excludes restricted sessions. Raw authentication lookup and MFA completion remain available. | `207d38dc1`; real HTTP password-login/MFA regression added. Main GREEN pending at report time; no passing backend claim. See [accounts contract](../../specs/cobalt-accounts.md). |
| Render / HTML documents | Successful WWS HTML-block responses add `Content-Security-Policy: sandbox allow-scripts`, without `allow-same-origin`; code blocks remain unchanged. | `4d47d4202`, contract proof `96efbdcc5`; router 3/3 and Chromium 1/1 RED/GREEN. Renderer HTML `example.com` stub remains a known functional gap, not fixed here. |
| Files / media | HTML-block isolation preserves Page/View checks, denial ordering and private cache behavior. Uploaded-file responses are a separate boundary. | HTML router fixtures cover public/restricted blocks, not upload admission or an uploaded active-document exploit. See [media contract](../../specs/cobalt-media-routing.md). |
| Persistence / import | Reviewed imported-history duplicate handling, filter semantics, historical context and trusted import/rerender RPC reachability. Candidates below rejected under current contracts. | No new import fix or import regression proof in this work; no source-site writes. |
| API lifecycle / permissions | Reviewed candidate validation and mutation permission boundaries, including restricted request identity and trusted RPC exposure. | Restricted-identity backend regression still pending GREEN; sampled inspection does not establish every route safe. |

## Local proof

- HTML router: `cargo test handler::text_block_tests -- --ignored --nocapture` in `wws`, isolated Redis and HTTP Deepwell/S3 fixtures. GREEN at `4d47d4202`: 3/3; 36 unrelated tests filtered. Public HTML CSP, restricted authorization/cache ordering, unchanged code bytes/headers/cache. Temporary Redis instances stopped.
- HTML browser: `node --test tests/html-block-isolation.test.mjs` in `framerail`, exported actual WWS router response and Chromium. GREEN at `4d47d4202`: 1/1. Scripts execute; parent DOM/cookie access throws; message origin is `null`; API reads fail without CORS permission, including explicit credentialed fetch. RED demonstrated parent, cookie and API-response reads before isolation.
- Browser limits: two outgoing requests still observed. No cookie in this Chromium/SameSite=Lax fixture is not a general no-request/no-CSRF guarantee. Sandbox does not replace API CORS or CSRF protections. No renderer-wide or upload-wide safety claim.
- Profile: `node --test tests/user-edit.test.ts` in `framerail`: four cases cover restricted, missing-cookie, null-session and unrestricted saves; denied cases preserve saved profile state. Ledger records GREEN against owned-file hashes while HEAD was `96efbdcc5`; guard/test changes were subsequently committed as `d4f77d16d`. Existing missing-pnpm and route-prefix diagnostics remain; this is not a full build gate.
- Backend MFA: `deepwell/src/api/tests.rs::mfa_pending_session_has_no_viewer_or_request_actor_until_verified` covers real HTTP login, pending lookup, actor/viewer denial, recovery-code completion and unrestricted access. Test addition is committed at `207d38dc1`; runtime GREEN is pending, not inferred from the accounts spec checkbox.

Local evidence: `/tmp/cobalt-security-html-proof-ledger.md` and `/tmp/cobalt-security-profile-proof-ledger.md`. Main ledger `/tmp/cobalt-security-mfa-proof-ledger.md` was not yet available when this report was written. Temporary paths are local evidence pointers, not durable tracked artifacts. No broad tests, final verification gate, push or deployment performed by this documentation update.

## Rejected candidates

- Imported-history same-request duplicates already undergo validation; not an accepted defect.
- OR history filters are intentional; not a filter bug.
- Historical rendering with current context is intentional; not an accepted context leak.
- Trusted loopback import/rerender RPCs are not public exploits absent a demonstrated Framerail/gateway exposure. Trust boundary remains relevant; no universal RPC safety claim.

## Open gaps / candidates

- **Uploaded active HTML:** `wws/src/handler/file.rs` uses metadata MIME for public responses, without HTML-block sandbox; only download routes force attachment disposition. Same-origin active HTML may be possible if upload admission permits it. Admission and exploit not reproduced; unconfirmed, unchanged.
- **Renderer HTML stub:** `example.com` output remains a known functional gap; unchanged. WWS response isolation does not claim renderer completeness.
- **Credential logs:** candidate only; logging exposure and exploit prerequisites not confirmed. Not fixed or declared safe.
- **Backend MFA GREEN / production:** await main's regression result; no production rollout or deployed acceptance proof for any fix in this report.
