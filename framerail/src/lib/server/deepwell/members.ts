import { client } from "$lib/server/deepwell"

import type { RequestContext } from "$lib/server/load/request-ctx"

// Deepwell resolves the acting user from `context.sessionToken` and checks
// that they are an admin of `context.siteId`; no call names the actor.

export type MemberRole = "member" | "moderator" | "admin" | "root"

export interface SiteMember {
  user_id: number
  name: string
  slug: string
  email: string
  joined_at: string
  role: MemberRole
}

export interface InviteResult {
  user_id: number
  created: boolean
  emailed: boolean
}

export async function memberAdminList(context: RequestContext): Promise<SiteMember[]> {
  return client.request("member_admin_list", {}, context)
}

export async function memberAdminSetRole(
  userId: number,
  role: MemberRole,
  ipAddress: string,
  context: RequestContext
): Promise<null> {
  return client.request(
    "member_admin_set_role",
    { user_id: userId, role, ip_address: ipAddress },
    context
  )
}

export async function memberAdminRemove(
  userId: number,
  ipAddress: string,
  context: RequestContext
): Promise<null> {
  return client.request(
    "member_admin_remove",
    { user_id: userId, ip_address: ipAddress },
    context
  )
}

export async function memberAdminInvite(
  email: string,
  name: string | null,
  ipAddress: string,
  context: RequestContext
): Promise<InviteResult> {
  return client.request(
    "member_admin_invite",
    { email, name, ip_address: ipAddress },
    context
  )
}
