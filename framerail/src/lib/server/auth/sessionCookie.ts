import type { Cookies } from "@sveltejs/kit"

const TOKEN_COOKIE = "wikijump_token"
const REMEMBER_COOKIE = "wikijump_remember"
// Browsers cap persistent cookies at 400 days; remembered sessions re-set
// their cookies on each visit so they never reach it.
const REMEMBER_MAX_AGE = 60 * 60 * 24 * 400

const cookieOptions = {
  path: "/",
  httpOnly: true,
  secure: true,
  sameSite: "lax"
} as const

/**
 * Stores the session token. A remembered session survives closing the
 * browser; otherwise the cookie lasts until the browser session ends.
 */
export function setSessionCookie(cookies: Cookies, token: string, remember: boolean) {
  if (remember) {
    cookies.set(TOKEN_COOKIE, token, { ...cookieOptions, maxAge: REMEMBER_MAX_AGE })
    cookies.set(REMEMBER_COOKIE, "1", { ...cookieOptions, maxAge: REMEMBER_MAX_AGE })
  } else {
    cookies.set(TOKEN_COOKIE, token, cookieOptions)
    cookies.delete(REMEMBER_COOKIE, cookieOptions)
  }
}

/** Pushes a remembered session's cookies out another 400 days. */
export function refreshRememberedSession(cookies: Cookies) {
  const token = cookies.get(TOKEN_COOKIE)
  if (token && cookies.get(REMEMBER_COOKIE)) setSessionCookie(cookies, token, true)
}

export function clearSessionCookies(cookies: Cookies) {
  cookies.delete(TOKEN_COOKIE, cookieOptions)
  cookies.delete(REMEMBER_COOKIE, cookieOptions)
}
