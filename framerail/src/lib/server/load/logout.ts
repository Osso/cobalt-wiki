import defaults from "$lib/defaults"
import { requireDeepwellError } from "$lib/deepwell-errors"

import { parseAcceptLangHeader } from "$lib/locales"
import { authLogout } from "$lib/server/auth/logout"
import { translate } from "$lib/server/deepwell/translate"
import { loadSiteInfo } from "$lib/server/load/site-info"
import { fail } from "@sveltejs/kit"

import type { PreloadDataAsync } from "$lib/server/deepwell/views"
import type { TranslateKeys } from "$lib/types"
import type { Cookies, RequestEvent } from "@sveltejs/kit"

export async function loadLogoutPage(
  request: Request,
  cookies: Cookies,
  preloadData: PreloadDataAsync
) {
  // Set up parameters
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const { siteId } = loadSiteInfo(request.headers)
  const sessionToken = cookies.get("wikijump_token")

  const parentData = await preloadData()
  const locales = parentData.locales

  const isLoggedIn = Boolean(parentData.user_session)

  const translateKeys: TranslateKeys = {
    ...defaults.translateKeys,

    // Page actions
    "cancel": {},
    "logout": {},

    // misc
    "logout.toast": {}
  }

  const internationalization = await translate(locales, translateKeys)

  // Return to page for rendering
  return { isLoggedIn, internationalization }
}

export async function logoutAction({ cookies, request }: RequestEvent) {
  const sessionToken = cookies.get("wikijump_token")

  try {
    // If we can't get the session token, the user must have logged out already
    if (!sessionToken) {
      const locales = parseAcceptLangHeader(request)
      if (!locales.includes(defaults.fallbackLocale)) {
        locales.push(defaults.fallbackLocale)
      }
      const translateStrings = await translate(locales, {
        "error-api.NOT_LOGGED_IN": {}
      })
      return fail(400, {
        message: translateStrings?.["error-api.NOT_LOGGED_IN"],
        code: undefined,
        data: undefined
      })
    }

    await authLogout(sessionToken)

    cookies.delete("wikijump_token", {
      path: "/",
      httpOnly: true,
      secure: true,
      sameSite: "lax"
    })

    return { success: true }
  } catch (caught) {
    const error = requireDeepwellError(caught)
    return fail(400, {
      message: error.message,
      code: error.code,
      data: error.data
    })
  }
}
