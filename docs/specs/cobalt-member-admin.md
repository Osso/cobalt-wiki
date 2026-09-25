# Cobalt member administration

Site admins manage members from `/-/admin/members`, the replica of Wikidot's `_admin#/members-list`. Membership is the `site-member` relation (its `created_at` is the join date); roles are the site's `member`, `moderator`, `admin` and `root` grants in `user_role` (see [accounts](cobalt-accounts.md) for set-password links).

## What it must do

- [x] Only a user holding an active `admin` or `root` grant on the site can list members, change roles, remove or invite. Deepwell decides this from the request's session (`X-Deepwell-Session-Token`, resolved by Deepwell) and site (`X-Deepwell-Site-Id`); no parameter names the acting user. Anyone else, and a restricted (unfinished MFA) session, gets `PermissionDenied` 3106 and nothing changes.
- [x] The list shows every current member: name, profile link, email, highest role (root > admin > moderator > member) and join date. Tabs: All Members, Moderators, Administrators (admins and root).
- [x] A role change makes a member a plain member, a moderator or an admin: other moderator/admin grants are revoked, the `member` grant is kept. `root` is never assigned here (`BadRequest` 4000).
- [x] Removal revokes the member's managed roles and ends the membership (audit reason "removed by a site admin").
- [x] The root member cannot be changed or removed (`RootMemberProtected` 3110); an admin cannot change or remove their own membership (`OwnMembership` 3111). The page shows no options on those rows.
- [x] Invite by email: an address with no regular account creates one under the given name (`UserNameRequired` 4109 without a name) with a random password, adds the membership (`Invitation` by the admin) and the `member` role, then emails a set-password link through `PasswordTokenService::create_link`. An existing account only joins, without an email. A current member is refused (`SiteMemberExists` 2109) so the join date is kept. A failed email fails the whole RPC, whose transaction rolls back.
- [x] The header account menu links to the page ("Site members") only when `preload_view` reports `site_admin`.
- [x] A revoked role can be granted again: `user_role_grant` revives the `(user_id, role_id)` row instead of inserting a duplicate key.
- [x] The existing `[[module Join]]` button links to `/-/join`, retaining its escaped custom label; it no longer relies on Wikidot's absent dialog script.
- [ ] Membership applications have frontend support: signed-in guests can send a short message at `/-/join`; pending guests see their message; members cannot apply; Site members presents admin/root Approve/Reject controls. Rejecting permits a later application. The backend integration, local end-to-end verification, and public deployment remain pending; see [membership applications](cobalt-membership-applications.md) for the lifecycle.
- [ ] Not offered: bans, Wikidot's per-member "send private message".

## RPCs

All read the acting user and site from the request headers.

| Method | Params | Result |
|---|---|---|
| `member_admin_list` | none | `[{user_id, name, slug, email, joined_at, role}]` |
| `member_admin_set_role` | `user_id`, `role` (`member`/`moderator`/`admin`), `ip_address` | null |
| `member_admin_remove` | `user_id`, `ip_address` | null |
| `member_admin_invite` | `email`, `name` (optional), `ip_address` | `{user_id, created, emailed}` |

`preload_view` also returns `site_admin`. `user_role_revoke` (`revoke_role_from_user` in `endpoints/role.rs`) stays unregistered: the existing role RPCs trust their loopback caller, and the page does not need it.

## Implementation inventory

- `deepwell/src/services/member_admin.rs`, `deepwell/src/endpoints/member_admin.rs`, `deepwell/src/api.rs` — RPCs and the admin check.
- `deepwell/src/services/role/service.rs` — grant upsert. `deepwell/src/services/view/` — `site_admin`. `deepwell/src/error/error_type.rs` — 2109, 3110, 3111, 4109.
- `framerail/src/lib/server/deepwell/members.ts`, `framerail/src/lib/server/load/members.ts`, `framerail/src/routes/[x+2d]/admin/members/`, `framerail/src/lib/component/LoginStatus.svelte`, `framerail/src/lib/page-layout.ts`.

## Tests asserting this spec

- `deepwell/tests/member_admin.rs` (DB-backed, `cobalt_test`): anonymous, member, moderator and restricted-admin callers refused by all four RPCs with no change; listing fields and highest role; promote/demote/re-promote (fails without the upsert); removal, root and self protection; invite creating the account and emailing a link (fake Mailgun) that redeems, existing account joining without email, current member refused; new-account invite without Mailgun fails; preload `site_admin`. 7/7 passed at `b335860`.
- `framerail/tests/members.test.ts` (mocked Deepwell): signed-out and non-admin views, session/site headers and params of every RPC, rows and options, error messages, no backend call without a session, header link for admins only. `framerail/tests/page-layout.test.ts`: site theme. 10/10 and 2/2 passed at `7bed3c2`.
- `deepwell/vendor/ftml/src/render/handle.rs::join_links_to_membership_application` verifies the native link and default/custom escaped labels (1/1 passed).
- `framerail/tests/join.test.ts` covers signed-out, guest, pending and member page states; actor/site request context; message validation; readable backend refusals. 4/4 passed with 2/2 page-layout tests during development.
- `framerail/tests/members.test.ts` also covers the pending application queue, safely rendered messages, approve/reject RPC context, malformed decisions, and signed-out/unauthorized review; 12/12 passed during application UI development.
- Pending proof: backend integration, local end-to-end application flow, and public deployment.
