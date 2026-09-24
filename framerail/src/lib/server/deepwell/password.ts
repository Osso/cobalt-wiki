import { client } from "$lib/server/deepwell"

export async function passwordTokenRedeem(
  token: string,
  password: string,
  ipAddress: string
): Promise<null> {
  return client.request("password_token_redeem", {
    token,
    password,
    ip_address: ipAddress
  })
}

export async function passwordResetRequest(email: string, siteId: number): Promise<null> {
  return client.request("password_reset_request", { email, site_id: siteId })
}
