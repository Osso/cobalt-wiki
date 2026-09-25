# Cobalt member applications

A signed-in guest can request site membership with a short message. Site admins review pending requests; backend behavior lives in `deepwell/src/services/member_application.rs`. See [member administration](cobalt-member-admin.md) for the existing membership and role contract.

## What it must do

- [x] `member_application_get` returns `{is_member, application}` for the requesting session and site. `application` is null or `{message, created_at}`; signed-out and restricted sessions are denied.
- [x] `member_application_submit` requires `{message, ip_address}`. Trim whitespace; reject empty messages, messages over 2000 characters, duplicate pending requests, and current members. Submission leaves the user a guest without page edit permission.
- [x] `member_application_list` returns `[{user_id, user_name, message, created_at}]` for pending requests on the current site, only to site admins.
- [x] `member_application_decide` requires `{user_id, accept, ip_address}` and site-admin authorization. Acceptance atomically creates membership with `Accepted(admin_user_id)`, grants `member`, and consumes the pending request; rejection only consumes the request so the guest can reapply. Missing requests are rejected.
- [x] Site and acting user are always trusted request context, never parameters; restricted sessions cannot apply or review. Requests on another site cannot access a site's applications.
- [x] Concurrent submissions and reviews for the same applicant serialize at the persisted user row; only one pending request can be created or consumed.

## How it works

- [Member administration](cobalt-member-admin.md) documents existing membership, permissions, and roles.

## Implementation inventory

- `deepwell/src/services/member_application.rs` — session authorization, relation lifecycle, membership acceptance.
- `deepwell/src/endpoints/member_application.rs`, `deepwell/src/api.rs` — RPC parsing and registration.

## Tests asserting this spec

- `deepwell/tests/member_application.rs` — DB-backed application lifecycle, permissions, validation, session/site isolation, and separate-transaction concurrency.

## Known gaps (current cycle)

- [ ] The Join form and Site members moderation queue are locally integrated with this backend. The backend DB suite passed 6/6; independent browser, lint, type, and check verification remains pending. No production deployment has occurred.

## Out of scope

- Email delivery, password-based joining, application history, and UI implementation — not part of backend application lifecycle.
