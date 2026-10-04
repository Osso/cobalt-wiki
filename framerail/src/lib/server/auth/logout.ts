import { client } from "$lib/server/deepwell"

export async function authLogout(sessionToken: string): Promise<void> {
  return client.request("logout", [sessionToken])
}

/** Ends every session of this user except the given one. */
export async function authInvalidateOtherSessions(
  sessionToken: string,
  userId: number
): Promise<number> {
  return client.request("session_invalidate_others", {
    session_token: sessionToken,
    user_id: userId
  })
}
