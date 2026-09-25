import defaults from "$lib/defaults"
import { requireDeepwellError } from "$lib/deepwell-errors"
import {
  memberApplicationGet,
  memberApplicationSubmit,
  type ApplicationStatus
} from "$lib/server/deepwell/applications"
import { translate } from "$lib/server/deepwell/translate"
import type { PreloadDataAsync } from "$lib/server/deepwell/views"
import { loadSiteChrome } from "$lib/server/load/site-chrome"
import { loadSiteInfo } from "$lib/server/load/site-info"
import { fail, type Cookies, type RequestEvent } from "@sveltejs/kit"

export async function loadJoinPage(
  request: Request,
  cookies: Cookies,
  preloadData: PreloadDataAsync
) {
  const { siteId } = loadSiteInfo(request.headers)
  const sessionToken = cookies.get("wikijump_token")
  const parentData = await preloadData()
  const signedIn = Boolean(parentData.user_session && sessionToken)
  const chrome = await loadSiteChrome(siteId, sessionToken, parentData)
  const internationalization = await translate(parentData.locales, {
    ...defaults.translateKeys,
    ...chrome.footerKeys
  })
  const status: ApplicationStatus | null = signedIn
    ? await memberApplicationGet({ siteId, sessionToken })
    : null
  return {
    compiled_top_bar_html: chrome.compiled_top_bar_html,
    internationalization,
    signedIn,
    status
  }
}

export async function applyForMembership({
  request,
  cookies,
  getClientAddress
}: RequestEvent) {
  const sessionToken = cookies.get("wikijump_token")
  if (!sessionToken)
    return fail(401, { message: "Sign in before applying for membership." })
  const form = await request.formData()
  const field = form.get("message")
  const message = typeof field === "string" ? field.trim() : ""
  if (!message || [...message].length > 2000) {
    return fail(400, {
      message: "Enter an application message of 1–2,000 characters.",
      applicationMessage: message
    })
  }
  const { siteId } = loadSiteInfo(request.headers)
  try {
    await memberApplicationSubmit(message, getClientAddress(), { siteId, sessionToken })
    return {
      submitted: true,
      message:
        "Your application is awaiting administrator review. You remain a guest until approved."
    }
  } catch (caught) {
    const error = requireDeepwellError(caught)
    const messages: Record<number, string> = {
      3106: "You cannot apply with this session. Complete sign-in and try again.",
      2109: "You are already a member of this site.",
      4000: "Your application could not be submitted. Refresh to check whether one is already pending."
    }
    return fail(error.code === 3106 ? 403 : messages[error.code] ? 400 : 500, {
      message:
        messages[error.code] ??
        "The application could not be submitted. Please try again later.",
      applicationMessage: message
    })
  }
}
