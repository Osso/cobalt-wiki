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
- `framerail/src/routes/[x+2d]/join/`, `framerail/src/routes/[x+2d]/admin/members/` — Join form and Site members approval queue.

## Tests asserting this spec

- `deepwell/tests/member_application.rs` — DB-backed application lifecycle, permissions, validation, session/site isolation, and separate-transaction concurrency (6/6).
- `deepwell/vendor/ftml/src/render/handle.rs::join_links_to_membership_application` — native Join link rendering (1/1).
- `framerail/tests/join.test.ts` — signed-out, guest, pending and member Join states; validation and request context (4/4).
- `framerail/tests/members.test.ts` — pending queue, safe messages, decisions, malformed decisions and authorization (12/12).

## Deployment and proof

- [x] Local end-to-end lifecycle verification passed 1/1: a guest remains pending, rejection permits reapplication, approval grants editor access, removal cleans up membership, and the flow performs no page saves. Evidence: `/tmp/claude/cobalt-membership-applications-followup-ledger-2026-09-25.md`.
- [x] Production deployment completed September 25, 2026 at 07:30:59 UTC. `./install/dev-deploy.sh` deployed `all` at `0d9ebcca4153`; its optimized backend took 9m09, production source matched `b8127c096` before these docs edits, and both units were active.
- [x] Anonymous production calls to `member_application_get` and `member_application_list` returned `PermissionDenied` 3106, proving the deployed RPC routes enforce their authorization boundary.
- [x] Read-only public browser proof follows the homepage Join link to `/-/join`; an existing guest sees the application form (main inspected `/tmp/cobalt-production-membership-join.jpg`). Anonymous Join GET returns 200 with sign-in/account links and approval requirement; anonymous Site members GET withholds admin access.
- [ ] No public write-lifecycle proof is claimed: applying, rejecting, approving, and resulting editor access were exercised locally, not against real production accounts.

## Out of scope

- Email delivery, password-based joining, and application history.
