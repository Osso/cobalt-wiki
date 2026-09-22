import defaults from "$lib/defaults"
import { requireDeepwellError } from "$lib/deepwell-errors"

import { authGetSession } from "$lib/server/auth/getSession"
import { authLogin } from "$lib/server/auth/login"
import { translate } from "$lib/server/deepwell/translate"
import { loadSiteInfo } from "$lib/server/load/site-info"
import { fail } from "@sveltejs/kit"
import { superValidate } from "sveltekit-superforms"
import { valibot } from "sveltekit-superforms/adapters"
import { minLength, object, pipe, string } from "valibot"

import type { PreloadDataAsync } from "$lib/server/deepwell/views"
import type { TranslateKeys } from "$lib/types"
import type { Cookies, RequestEvent } from "@sveltejs/kit"

export async function loadLoginPage(
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
    "login": {},

    // misc
    "specifier": {},
    "password": {},
    "login.toast": {},
    "forgot-password": {},
    "remember-me": {},
    "create-account": {}
  }

  const internationalization = await translate(locales, translateKeys)

  // superform
  const loginForm = await superValidate(valibot(loginSchema))

  // Return to page for rendering
  return { isLoggedIn, internationalization, loginForm }
}

export async function loginAction({ request, getClientAddress, cookies }: RequestEvent) {
  const form = await superValidate(request, valibot(loginSchema))

  if (!form.valid) {
    return fail(400, { form })
  }

  const userAgent: string = request.headers.get("User-Agent") ?? ""
  const ipAddress: string = getClientAddress()

  const { data } = form

  try {
    const res = await authLogin(data.nameOrEmail, data.password, ipAddress, userAgent)

    if (res.session_token) {
      const session = await authGetSession(res.session_token)
      cookies.set("wikijump_token", res.session_token, {
        path: "/",
        httpOnly: true,
        secure: true,
        sameSite: "lax",
        expires: new Date(session.expires_at)
      })
    }

    return { form, session_token: res.session_token, isLoggedIn: true }
  } catch (caught) {
    const error = requireDeepwellError(caught)
    return fail(500, {
      form,
      message: error?.message,
      code: error?.code,
      data: error?.data
    })
  }
}

const loginSchema = object({
  nameOrEmail: pipe(string(), minLength(1)),
  password: pipe(string(), minLength(1))
})
