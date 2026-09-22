import defaults from "$lib/defaults"

import { parseAcceptLangHeader } from "$lib/locales"

import { preloadView } from "$lib/server/deepwell/views"
import { loadSiteInfo } from "$lib/server/load/site-info"
import { sanitizeUserData } from "$lib/user-data"

import type { Cookies } from "@sveltejs/kit"

/**
 * Loads common data that will be used in all routes, including site info,
 * user session and locales
 */
export async function loadPreload(request: Request, cookies: Cookies) {
  // Set up parameters
  const { siteId } = loadSiteInfo(request.headers)
  const sessionToken = cookies.get("wikijump_token")
  let locales = parseAcceptLangHeader(request)

  // Request data from backend
  // Includes fallback locale in case there is no Accept-Language header
  const response = await preloadView(
    siteId,
    [...locales, defaults.fallbackLocale],
    sessionToken
  )

  if (response.user_session?.user.locales) {
    locales = [
      ...response.user_session.user.locales,
      ...locales.filter((locale) => !response.user_session?.user.locales.includes(locale))
    ]
  }

  if (response?.site?.locale && !locales.includes(response.site.locale)) {
    locales.push(response.site.locale)
  }

  if (!locales.includes(defaults.fallbackLocale)) locales.push(defaults.fallbackLocale)

  const userSession = response.user_session
    ? {
        ...response.user_session,
        user: sanitizeUserData(response.user_session.user, false)
      }
    : null

  // Handover only sanitized user data to subsequent requests for rendering.
  return { ...response, user_session: userSession, locales }
}
