import defaults from "$lib/defaults"
import { requireDeepwellError } from "$lib/deepwell-errors"

import { passwordResetRequest, passwordTokenRedeem } from "$lib/server/deepwell/password"
import { translate } from "$lib/server/deepwell/translate"
import { loadSiteChrome } from "$lib/server/load/site-chrome"
import { loadSiteInfo } from "$lib/server/load/site-info"
import { fail } from "@sveltejs/kit"

import type { PreloadDataAsync } from "$lib/server/deepwell/views"
import type { Cookies, RequestEvent } from "@sveltejs/kit"

// Deepwell's error codes for a password link that cannot be used.
const LINK_ERRORS: Record<number, string> = {
  3007: "This link is not valid. Ask for a new one below.",
  3008: "This link has expired. Ask for a new one below.",
  3009: "This link has already been used. Sign in with your password, or ask for a new link below."
}

/** Site header and footer for the set-password and forgot-password pages. */
export async function loadPasswordPage(
  request: Request,
  cookies: Cookies,
  preloadData: PreloadDataAsync
) {
  const { siteId } = loadSiteInfo(request.headers)
  const parentData = await preloadData()
  const chrome = await loadSiteChrome(siteId, cookies.get("wikijump_token"), parentData)
  const internationalization = await translate(parentData.locales, {
    ...defaults.translateKeys,
    ...chrome.footerKeys
  })
  return { compiled_top_bar_html: chrome.compiled_top_bar_html, internationalization }
}

export async function setPasswordAction({
  request,
  params,
  getClientAddress
}: RequestEvent) {
  const form = await request.formData()
  const password = formText(form, "newPassword")
  if (!password) return fail(400, { message: "Enter a new password." })
  if (password !== formText(form, "confirmPassword")) {
    return fail(400, { message: "The passwords do not match." })
  }

  try {
    await passwordTokenRedeem(params.token ?? "", password, getClientAddress())
    return { saved: true }
  } catch (caught) {
    const error = requireDeepwellError(caught)
    const message = LINK_ERRORS[error.code]
    if (message) return fail(400, { message, linkUnusable: true })
    return fail(500, { message: error.message })
  }
}

/**
 * The reply is the same whether or not the address has an account, so the
 * page cannot be used to find out who is a member.
 */
export async function forgotPasswordAction({ request }: RequestEvent) {
  const form = await request.formData()
  const email = formText(form, "email").trim()
  if (!email) return fail(400, { message: "Enter your email address." })

  const { siteId } = loadSiteInfo(request.headers)
  try {
    await passwordResetRequest(email, siteId)
    return { sent: true }
  } catch (caught) {
    requireDeepwellError(caught)
    return fail(500, { message: "The email could not be sent. Try again later." })
  }
}

function formText(form: FormData, name: string): string {
  const value = form.get(name)
  return typeof value === "string" ? value : ""
}
