import { Layout } from "$lib/types"

// Account pages a Wikidot site's visitors reach from its header use the site's theme.
const SITE_THEMED_ROUTES = new Set([
  "/[x+2d]/forgot-password",
  "/[x+2d]/login",
  "/[x+2d]/logout",
  "/[x+2d]/register",
  "/[x+2d]/set-password/[token]",
  "/[x+2d]/settings"
])

interface LayoutSource {
  route: { id: string | null }
  data: {
    page?: { layout?: Layout | null } | null
    site?: { layout?: Layout | null } | null
  }
}

/**
 * The layout a route renders with. Pure, so server rendering picks the
 * same layout as the browser and the page does not flash Wikijump's layout
 * first.
 */
export function pageLayout({ route, data }: LayoutSource): Layout {
  if (route.id?.startsWith("/[x+2d]/") && !SITE_THEMED_ROUTES.has(route.id)) {
    // this is a special page, use Wikijump layout
    return Layout.WIKIJUMP
  }
  return data?.page?.layout ?? data?.site?.layout ?? Layout.WIKIJUMP
}
