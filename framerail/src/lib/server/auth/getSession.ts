import { client } from "$lib/server/deepwell"
import { error } from "@sveltejs/kit"

export async function authGetSession(sessionToken: string | undefined): Promise<{
  session_token: string
  user_id: number
  created_at: string
  expires_at: string
  ip_address: string
  user_agent: string
  restricted: boolean
}> {
  const session = await client.request("session_get", [sessionToken ?? ""])
  if (!session) error(401, "You are not signed in.")
  if (session.restricted) {
    error(403, "Complete multi-factor authentication before performing this action.")
  }
  return session
}
