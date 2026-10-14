import defaults from "$lib/defaults"
import { parseAcceptLangHeader } from "$lib/locales"
import { pageView } from "$lib/server/deepwell/views"
import { loadSiteInfo } from "$lib/server/load/site-info"
import { error } from "@sveltejs/kit"
import type { RequestEvent } from "@sveltejs/kit"

/** Public pane reads may only query their authorized, routed page. */
export async function authorizePageRead(
  { request, params, cookies }: RequestEvent,
  target: { siteId?: number; pageId: number }
) {
  const { siteId } = loadSiteInfo(request.headers)
  if (target.siteId !== undefined && target.siteId !== siteId) {
    error(403, "Page target does not match this site.")
  }
  const route =
    params.slug || params.extra ? { slug: params.slug, extra: params.extra } : null
  const locales = [...parseAcceptLangHeader(request), defaults.fallbackLocale]
  const view = await pageView(siteId, locales, route, cookies.get("wikijump_token"))
  if (view.type === "permissions") error(403, "Page view denied.")
  if (view.type === "missing") error(404, "Page not found.")
  if (view.data.page.page_id !== target.pageId) {
    error(403, "Page target does not match this route.")
  }
}
