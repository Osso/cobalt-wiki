import { loadPasswordPage, setPasswordAction } from "$lib/server/load/password"

export async function load({ request, cookies, parent }) {
  return loadPasswordPage(request, cookies, parent)
}

export const actions = { default: setPasswordAction }
