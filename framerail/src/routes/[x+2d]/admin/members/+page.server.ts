import {
  loadMembersPage,
  memberInviteAction,
  memberRemoveAction,
  memberRoleAction
} from "$lib/server/load/members"

export async function load({ request, cookies, parent }) {
  return loadMembersPage(request, cookies, parent)
}

export const actions = {
  role: memberRoleAction,
  remove: memberRemoveAction,
  invite: memberInviteAction
}
