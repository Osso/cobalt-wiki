import defaults from "$lib/defaults"
import {
  memberApplicationList,
  memberApplicationDecide,
  type PendingApplication
} from "$lib/server/deepwell/applications"
import { requireDeepwellError } from "$lib/deepwell-errors"

import {
  memberAdminInvite,
  memberAdminList,
  memberAdminRemove,
  memberAdminSetRole
} from "$lib/server/deepwell/members"
import { translate } from "$lib/server/deepwell/translate"
import { loadSiteChrome } from "$lib/server/load/site-chrome"
import { loadSiteInfo } from "$lib/server/load/site-info"
import { fail } from "@sveltejs/kit"

import type { MemberRole, SiteMember } from "$lib/server/deepwell/members"
import type { PreloadDataAsync } from "$lib/server/deepwell/views"
import type { Cookies, RequestEvent } from "@sveltejs/kit"

// Deepwell's error code when the session user is not an admin of the site.
const PERMISSION_DENIED = 3106
// Mailgun failures are high-level errors, found only in the code trace.
const EMAIL_SEND = 1210
// Messages for the Deepwell errors an admin can cause.
const ACTION_ERRORS: Record<number, string> = {
  [PERMISSION_DENIED]: "Only administrators of this site can manage its members.",
  2002: "That user is not a member of this site.",
  2003: "That user does not exist.",
  2100: "An account with that name already exists. Choose another name.",
  2109: "That account is already a member of this site.",
  3110: "The site owner cannot be changed or removed.",
  3111: "You cannot change or remove your own membership.",
  4000: "That application is no longer pending. Refresh the list.",
  4102: "Enter the email address to invite.",
  4105: "That email address is not valid.",
  4106: "That email address is not allowed.",
  4109: "No account uses that email address. Enter a name for the new account."
}
const ASSIGNABLE_ROLES: readonly MemberRole[] = ["member", "moderator", "admin"]

type Access = "signed-out" | "denied" | "admin"

/**
 * The members list, which Deepwell returns only to an admin of the site.
 * Other visitors get `access` saying why there is none.
 */
export async function loadMembersPage(
  request: Request,
  cookies: Cookies,
  preloadData: PreloadDataAsync
) {
  const { siteId } = loadSiteInfo(request.headers)
  const sessionToken = cookies.get("wikijump_token")
  const parentData = await preloadData()
  const chrome = await loadSiteChrome(siteId, sessionToken, parentData)
  const internationalization = await translate(parentData.locales, {
    ...defaults.translateKeys,
    ...chrome.footerKeys
  })

  const viewer = parentData.user_session?.user
  let access: Access = viewer && sessionToken ? "admin" : "signed-out"
  let members: SiteMember[] = []
  let applications: PendingApplication[] = []
  if (access === "admin") {
    try {
      members = await memberAdminList({ sessionToken, siteId })
      applications = await memberApplicationList({ sessionToken, siteId })
    } catch (caught) {
      if (requireDeepwellError(caught).code !== PERMISSION_DENIED) throw caught
      access = "denied"
    }
  }

  return {
    compiled_top_bar_html: chrome.compiled_top_bar_html,
    internationalization,
    access,
    viewerId: viewer?.user_id ?? null,
    members,
    applications
  }
}

export async function memberApplicationDecisionAction(event: RequestEvent) {
  const form = await event.request.formData()
  const userId = Number(formText(form, "userId"))
  const decision = formText(form, "decision")
  if (
    !Number.isSafeInteger(userId) ||
    userId <= 0 ||
    !["approve", "reject"].includes(decision)
  ) {
    return fail(400, { message: "Choose an application and approve or reject it." })
  }
  return runAdminAction(event, async (ip, context) => {
    await memberApplicationDecide(userId, decision === "approve", ip, context)
    return decision === "approve"
      ? "Application approved. The applicant is now a member."
      : "Application rejected. The applicant remains a guest and may apply again."
  })
}

export async function memberRoleAction(event: RequestEvent) {
  const form = await event.request.formData()
  const userId = Number(formText(form, "userId"))
  const role = formText(form, "role") as MemberRole
  if (!ASSIGNABLE_ROLES.includes(role)) {
    return fail(400, { message: "Choose Member, Moderator or Administrator." })
  }
  return runAdminAction(event, async (ip, context) => {
    await memberAdminSetRole(userId, role, ip, context)
    return `${formText(form, "name")} is now ${ROLE_LABELS[role]}.`
  })
}

export async function memberRemoveAction(event: RequestEvent) {
  const form = await event.request.formData()
  const userId = Number(formText(form, "userId"))
  return runAdminAction(event, async (ip, context) => {
    await memberAdminRemove(userId, ip, context)
    return `${formText(form, "name")} is no longer a member.`
  })
}

export async function memberInviteAction(event: RequestEvent) {
  const form = await event.request.formData()
  const email = formText(form, "email").trim()
  const name = formText(form, "name").trim()
  if (!email) return fail(400, { message: "Enter the email address to invite." })
  return runAdminAction(event, async (ip, context) => {
    const result = await memberAdminInvite(email, name || null, ip, context)
    return result.created
      ? `Invited ${email}: the new account was emailed a link to choose a password.`
      : `The existing account with ${email} is now a member.`
  })
}

const ROLE_LABELS: Record<MemberRole, string> = {
  member: "a member",
  moderator: "a moderator",
  admin: "an administrator",
  root: "the site owner"
}

/**
 * Runs a member change for the session cookie's user. Deepwell checks that
 * this user is a site admin; the form never names the acting user.
 */
async function runAdminAction(
  { request, cookies, getClientAddress }: RequestEvent,
  change: (
    ipAddress: string,
    context: { sessionToken: string; siteId: number }
  ) => Promise<string>
) {
  const sessionToken = cookies.get("wikijump_token")
  if (!sessionToken) return fail(401, { message: "You are not signed in." })
  const { siteId } = loadSiteInfo(request.headers)
  try {
    const message = await change(getClientAddress(), { sessionToken, siteId })
    return { saved: true, message }
  } catch (caught) {
    const error = requireDeepwellError(caught)
    const message = ACTION_ERRORS[error.code]
    if (message) return fail(error.code === PERMISSION_DENIED ? 403 : 400, { message })
    if (codeTrace(error.data).includes(EMAIL_SEND)) {
      return fail(500, {
        message: "The invite email could not be sent. Nothing was changed."
      })
    }
    return fail(500, { message: `The change failed: ${error.message}` })
  }
}

/** Deepwell's codes of every error in the chain, outermost first. */
function codeTrace(data: unknown): number[] {
  if (typeof data === "object" && data !== null && "code_trace" in data) {
    const trace = data.code_trace
    if (Array.isArray(trace)) return trace.filter((code) => typeof code === "number")
  }
  return []
}

function formText(form: FormData, name: string): string {
  const value = form.get(name)
  return typeof value === "string" ? value : ""
}
