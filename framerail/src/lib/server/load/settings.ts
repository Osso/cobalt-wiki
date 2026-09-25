import defaults from "$lib/defaults"
import { requireDeepwellError } from "$lib/deepwell-errors"

import { authLogin } from "$lib/server/auth/login"
import { authLogout } from "$lib/server/auth/logout"
import { translate } from "$lib/server/deepwell/translate"
import { userEdit, userView } from "$lib/server/deepwell/user"
import {
  watchingPreferencesGet,
  watchingPreferencesSet,
  watchingSubscriptions
} from "$lib/server/deepwell/watching"
import { loadSiteChrome } from "$lib/server/load/site-chrome"
import { loadSiteInfo } from "$lib/server/load/site-info"
import { userProfileFields } from "$lib/user-data"
import { fail } from "@sveltejs/kit"

import type { PreloadDataAsync } from "$lib/server/deepwell/views"
import type { UserModel } from "$lib/types"
import type { Cookies, RequestEvent } from "@sveltejs/kit"

// Deepwell's error code for a wrong name or password on login.
const INVALID_AUTHENTICATION = 3000

// Renames use up name changes, so the name stays on the profile page.
const PROFILE_FIELDS = [
  "realName",
  "gender",
  "birthday",
  "location",
  "website",
  "userPage",
  "biography"
] as const

type Section = "profile" | "email" | "password"
type AccountEdits = Parameters<typeof userEdit>[2]

export async function loadSettingsPage(
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

  const user = parentData.user_session?.user
  const sessionToken = cookies.get("wikijump_token")
  const [watchingPreferences, watchingSubscriptionsList] =
    user && sessionToken
      ? await Promise.all([
          watchingPreferencesGet({ sessionToken, siteId }),
          watchingSubscriptions(siteId, { sessionToken, siteId })
        ])
      : [null, null]
  return {
    compiled_top_bar_html: chrome.compiled_top_bar_html,
    internationalization,
    account: user ? { name: user.name, slug: user.slug, email: user.email } : null,
    profile: user ? userProfileFields(user) : null,
    watchingPreferences,
    watchingSubscriptions: watchingSubscriptionsList
  }
}

export async function settingsProfileAction(event: RequestEvent) {
  const form = await event.request.formData()
  const edits = Object.fromEntries(
    PROFILE_FIELDS.filter((field) => typeof form.get(field) === "string").map((field) => [
      field,
      formText(form, field)
    ])
  )
  return saveSettings(event, "profile", null, edits)
}

export async function settingsEmailAction(event: RequestEvent) {
  const form = await event.request.formData()
  const email = formText(form, "email").trim()
  if (!email) return fail(400, { section: "email", message: "Enter an email address." })
  return saveSettings(event, "email", formText(form, "currentPassword"), { email })
}

export async function settingsWatchingAction({ request, cookies }: RequestEvent) {
  const sessionToken = cookies.get("wikijump_token")
  if (!sessionToken) {
    return fail(401, { section: "watching", message: "You are not signed in." })
  }
  const form = await request.formData()
  const preferences = {
    email_enabled: form.get("email_enabled") === "on",
    auto_watch: form.get("auto_watch") === "on"
  }
  const { siteId } = loadSiteInfo(request.headers)
  try {
    await watchingPreferencesSet(preferences, { sessionToken, siteId })
    return { section: "watching", saved: true, message: "Watching preferences saved." }
  } catch (caught) {
    const error = requireDeepwellError(caught)
    return fail(400, { section: "watching", message: error.message })
  }
}

export async function settingsPasswordAction(event: RequestEvent) {
  const form = await event.request.formData()
  const password = formText(form, "newPassword")
  if (!password) {
    return fail(400, { section: "password", message: "Enter a new password." })
  }
  if (password !== formText(form, "confirmPassword")) {
    return fail(400, { section: "password", message: "The new passwords do not match." })
  }
  return saveSettings(event, "password", formText(form, "currentPassword"), { password })
}

/**
 * Applies edits to the account of the session cookie's user. Email and
 * password changes pass `currentPassword`, checked before anything is
 * saved.
 */
async function saveSettings(
  { request, cookies, getClientAddress }: RequestEvent,
  section: Section,
  currentPassword: string | null,
  edits: AccountEdits
) {
  const sessionToken = cookies.get("wikijump_token")
  if (!sessionToken) return fail(401, { section, message: "You are not signed in." })
  if (currentPassword === "") {
    return fail(400, { section, message: "Enter your current password." })
  }

  const ipAddress = getClientAddress()
  try {
    const user = await findSessionUser(request, sessionToken)
    if (!user) return fail(401, { section, message: "You are not signed in." })

    if (currentPassword !== null) {
      const userAgent = request.headers.get("User-Agent") ?? ""
      await checkPassword(user, currentPassword, ipAddress, userAgent)
    }

    await userEdit(user.user_id, ipAddress, edits)
    return { section, saved: true, message: "Your settings were saved." }
  } catch (caught) {
    const error = requireDeepwellError(caught)
    const message =
      error.code === INVALID_AUTHENTICATION
        ? "Your current password is incorrect."
        : error.message
    return fail(error.code === INVALID_AUTHENTICATION ? 400 : 500, { section, message })
  }
}

/**
 * Deepwell resolves the user from the session token, never from the
 * request.
 */
async function findSessionUser(
  request: Request,
  sessionToken: string
): Promise<UserModel | null> {
  const { siteId } = loadSiteInfo(request.headers)
  const view = await userView(siteId, [defaults.fallbackLocale], sessionToken)
  return view.type === "user_found" ? view.data.user : null
}

/**
 * Deepwell has no password check apart from login, so this signs in and
 * immediately ends that session.
 */
async function checkPassword(
  user: UserModel,
  password: string,
  ipAddress: string,
  userAgent: string
) {
  const { session_token } = await authLogin(user.slug, password, ipAddress, userAgent)
  await authLogout(session_token)
}

function formText(form: FormData, name: string): string {
  const value = form.get(name)
  return typeof value === "string" ? value : ""
}
