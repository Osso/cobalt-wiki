import { loadPage } from "$lib/server/load/page"
import { error } from "@sveltejs/kit"

import type { PageServerLoad } from "./$types"

export const load: PageServerLoad = async ({ params, request, cookies, parent }) => {
  const [slug, ...rest] = params.path?.split("/").filter(Boolean) ?? []
  const page = await loadPage(slug, rest.join("/"), request, cookies, parent)
  if (!("page" in page) || !page.page_revision) {
    error(500, "Print page response is missing page data")
  }
  const sourceUrl = new URL(`/${page.page.slug}`, request.url).href

  return { ...page, page_revision: page.page_revision, printView: true, sourceUrl }
}
