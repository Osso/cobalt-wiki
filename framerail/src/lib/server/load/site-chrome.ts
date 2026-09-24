import { pageView } from "$lib/server/deepwell/views"

import type { PreloadDataAsync } from "$lib/server/deepwell/views"
import type { TranslateKeys } from "$lib/types"

/**
 * Site header and footer data for account pages (login, logout, register),
 * which render in the site's layout but are not wiki pages: the top bar of
 * the site's front page and the license footer text.
 */
export async function loadSiteChrome(
  siteId: number,
  sessionToken: string | undefined,
  parentData: Awaited<ReturnType<PreloadDataAsync>>
) {
  const view = await pageView(siteId, parentData.locales, null, sessionToken)
  const footerKeys: TranslateKeys = {
    "footer-license-unless": {
      license: parentData.license_name,
      "license_url": parentData.license_url
    }
  }
  return { compiled_top_bar_html: view.data.compiled_top_bar_html ?? null, footerKeys }
}
