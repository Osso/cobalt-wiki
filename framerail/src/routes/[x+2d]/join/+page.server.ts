import { applyForMembership, loadJoinPage } from "$lib/server/load/join"

export async function load({ request, cookies, parent }) {
  return loadJoinPage(request, cookies, parent)
}

export const actions = { default: applyForMembership }
