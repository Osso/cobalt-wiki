import { fail } from "@sveltejs/kit"
import { client } from "$lib/server/deepwell"
import { loadRoutedPageView } from "./page-read"
import { deepwellErrorStatus, requireDeepwellError } from "$lib/deepwell-errors"
import { loadSiteInfo } from "./site-info"
import { getRequestContext } from "./request-ctx"

import type { RequestEvent } from "@sveltejs/kit"

export interface BacklinkPage {
  page_id: number
  slug: string
  title: string
}

export interface PageBacklinks {
  links: BacklinkPage[]
  inclusions: BacklinkPage[]
}

export async function pageBacklinksAction(event: RequestEvent) {
  try {
    const { siteId } = loadSiteInfo(event.request.headers)
    const view = await loadRoutedPageView(event)
    if (view.type === "permissions") {
      return fail(403, { message: "Page backlinks unavailable" })
    }
    if (view.type !== "found") {
      return fail(404, { message: "Page backlinks unavailable" })
    }
    const res: PageBacklinks = await client.request(
      "page_backlinks",
      { site_id: siteId, page_id: view.data.page.page_id },
      getRequestContext(event.locals)
    )
    return { res }
  } catch (cause) {
    const failure = requireDeepwellError(cause)
    return fail(deepwellErrorStatus(failure.code), failure)
  }
}
