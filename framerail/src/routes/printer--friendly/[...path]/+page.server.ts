import { loadPage } from "$lib/server/load/page"

import type { PageServerLoad } from "./$types"

export const load: PageServerLoad = async ({ params, request, cookies, parent }) => {
  const [slug, ...rest] = params.path?.split("/").filter(Boolean) ?? []
  const page = await loadPage(slug, rest.join("/") || undefined, request, cookies, parent)
  const sourceUrl = new URL(`/${page.page.slug}`, request.url).href

  return { ...page, printView: true, sourceUrl }
}
