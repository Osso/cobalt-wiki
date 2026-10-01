# Cobalt account color profiles

## Approved choices

- Colors only; no layout profiles.
- Preference saved per account, across devices.
- Classic: historical cyan tabs with black text. Current: existing tab backgrounds; slightly darker label text.
- Switch Luridel to Classic after deployed support is available.

## Design

Use one `color_profile` account field with values `classic` and `current`. Existing and new accounts retain Current unless explicitly changed. Extend the existing user-edit path and My account settings with an Appearance section; resolve the account from the authenticated session, not submitted identity.

Members open My account → Appearance (`/-/settings#appearance`), select Current or Classic under Tab colors, then submit Save appearance. Reopening settings displays the saved choice.

Pass the private account preference through the existing server-rendered layout. Scope navigation fill, hover, and label color variables to the Wikidot layout's profile attribute. Classic uses `#a8d7ea`, `#d6f3ff`, and black; Current retains `#bcc9f2` and `#e0e6fb`, with darker navy labels `#0b1a38`. Preserve rounded corners, layout, content, and unrelated styling.

For invalid form values, refuse the save and show an Appearance error. Backend validation must also reject unsupported values. Saving unrelated settings must not reset the preference.

## Deployment and verification

verified: 2026-09-30

- [x] Runtime/backend source `fdb02cd205250c0a3c2ac0eada2b9919d194e28d` deployed through `install/dev-deploy.sh`; exit 0, receipt `/tmp/cobalt-color-profiles-deploy.log`. Followup `b0ff79792f0ce92b0298f07025a6b5ec4e2e2611` changes only the page-data declaration and test typing. Final source gate `b50fe6492f81567e567eedc485a49089c5597b9e` also includes content-preservation test assertions; neither changes deployed runtime behavior.
- [x] Scoped gates pass: Svelte check 0 errors/0 warnings, ESLint/Prettier/Stylelint, Rust format/check. Initial declaration/test typing failures were repaired by the followup. Evidence: `/tmp/cobalt-color-verifier-report.md` and `/tmp/cobalt-color-verifier-proof.json`; final changed-test/type gate `/tmp/cobalt-color-profiles-final-proof.json` passes all nine contract requirements. Existing 10 ESLint `no-explicit-any` warnings, update-service complexity debt, and build notices remain; no adjacent cleanup claimed.
- [x] Saved regression proofs: frontend 27/27 in `/tmp/cobalt-color-profiles-green-final.log`; final component 1/1 in `/tmp/cobalt-color-profiles-content-preservation-green.log` supersedes its original component result, not an additional test, and asserts identical navigation/page content across profiles. Durable DB/input 2/2 in `/tmp/cobalt-color-profile-green.log` uses real committed transactions; serialization 1/1 in `/tmp/cobalt-color-profile-serialization.log`. Declaration/test-only followup and this docs update do not invalidate unchanged runtime/backend proof.
- [x] Authenticated local full-stack acceptance passes in `/tmp/cobalt-color-profiles-authenticated-smoke-diagnostic.log`: UI save/reload, browser-navigation raw SSR, second browser/account login retaining Classic, correct Classic/Current computed colors, and equal geometry. `/tmp/cobalt-profile-local-smoke.mjs` is a one-off acceptance artifact, not tracked regression coverage. The separate APIRequestContext raw HTTP probe omitted Secure cookies on loopback and saw anonymous Current; instrumentation confirmed browser navigation saw saved Classic. This was a probe issue, not a source failure.
- [x] Authorized production change saved exact `luridel`, user ID `121356`, from Current to Classic. Fresh `user_get` confirms Classic; unrelated private fields match baseline SHA256 `8915c3e7cdf475daec70653a3f7607160b93b7ba949949fbde7c2e1277b7abaa`. Receipt: `/tmp/cobalt-color-profiles-luridel-after.json`.

Production smoke `/tmp/cobalt-color-profiles-live-smoke.log` proves anonymous roster HTTP 200, first-render Current, existing Current background, and darker labels. Its Classic DOM-attribute probe checks the deployed CSS rule only: no production impersonation or authenticated Luridel page inspection is claimed. Local acceptance does not establish whole-replica readiness.

Temporary local test account removal is recorded in `/tmp/cobalt-color-profiles-test-cleanup.log`; temporary test service units were stopped. No production fixture account was created. These receipts establish the bounded deployment/account action, not broader theme or TOC parity.

Requirements and proof inventory live in the [feature spec](../specs/cobalt-color-profiles.md). No light/dark, System option, or browser-local preference is part of this profile change. TOC styling parity was an original-goal requirement left unchanged by the profile implementation, not optional work or an excused omission. Repair `e39d9f311` and its test-backed contract are tracked in the [TOC spec](../specs/cobalt-toc.md); deployed appearance, scrolling, and Luridel anchor acceptance pass; preference preservation and scoped verifier results remain pending.
