import { loadPage } from "$lib/server/load/page"
import { loadPageWatching } from "$lib/server/load/watching"
import { loadSiteInfo } from "$lib/server/load/site-info"
import { actions as pageActions } from "./[slug]/[...extra]/+page.server"

export async function load({ request, cookies, parent, locals }) {
  const page = await loadPage(undefined, undefined, request, cookies, parent)
  if ("page" in page) {
    locals.documentLayout = page.page.layout ?? (await parent()).site.layout
  }
  const { siteId } = loadSiteInfo(request.headers)
  const sessionToken = (await parent()).user_session
    ? cookies.get("wikijump_token")
    : null
  const watching =
    "page" in page
      ? await loadPageWatching(siteId, page.page, sessionToken ?? null)
      : null
  return { ...page, watching }
}

export const actions = pageActions
