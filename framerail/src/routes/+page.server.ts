import { loadPage } from "$lib/server/load/page"
import { actions as pageActions } from "./[slug]/[...extra]/+page.server"

export async function load({ request, cookies, parent, locals }) {
  const page = await loadPage(undefined, undefined, request, cookies, parent)
  if ("page" in page) {
    locals.documentLayout = page.page.layout ?? (await parent()).site.layout
  }
  return page
}

export const actions = pageActions
