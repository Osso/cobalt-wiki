import { client } from "$lib/server/deepwell"
import type { RequestContext } from "$lib/server/load/request-ctx"

export interface MembershipApplication {
  message: string
  created_at: string
}

export interface ApplicationStatus {
  is_member: boolean
  application: MembershipApplication | null
}

export interface PendingApplication extends MembershipApplication {
  user_id: number
  user_name: string
}

export function memberApplicationGet(
  context: RequestContext
): Promise<ApplicationStatus> {
  return client.request("member_application_get", {}, context)
}

export function memberApplicationSubmit(
  message: string,
  ipAddress: string,
  context: RequestContext
): Promise<unknown> {
  return client.request(
    "member_application_submit",
    { message, ip_address: ipAddress },
    context
  )
}

export function memberApplicationList(
  context: RequestContext
): Promise<{ applications: PendingApplication[] }> {
  return client.request("member_application_list", {}, context)
}

export function memberApplicationDecide(
  userId: number,
  accept: boolean,
  ipAddress: string,
  context: RequestContext
): Promise<unknown> {
  return client.request(
    "member_application_decide",
    { user_id: userId, accept, ip_address: ipAddress },
    context
  )
}
