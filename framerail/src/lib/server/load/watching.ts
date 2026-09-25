import { requireDeepwellError } from "$lib/deepwell-errors"
import {
  watchingSubscriptionSet,
  watchingSubscriptions
} from "$lib/server/deepwell/watching"
import { loadSiteInfo } from "$lib/server/load/site-info"
import { fail } from "@sveltejs/kit"

import type { WatchScope, WatchSubscription } from "$lib/server/deepwell/watching"
import type { RequestEvent } from "@sveltejs/kit"

type PageIdentity = { page_id: number; page_category_id: number }

export async function loadPageWatching(
  siteId: number,
  page: PageIdentity | null,
  sessionToken: string | null
): Promise<{
  siteId: number
  pageId: number
  categoryId: number
  subscriptions: WatchSubscription[]
} | null> {
  if (!page || !sessionToken) return null
  const subscriptions = await watchingSubscriptions(siteId, { sessionToken, siteId })
  return {
    siteId,
    pageId: page.page_id,
    categoryId: page.page_category_id,
    subscriptions
  }
}

export async function setSubscriptionAction({ request, cookies }: RequestEvent) {
  const sessionToken = cookies.get("wikijump_token")
  if (!sessionToken) return fail(401, { message: "You are not signed in." })

  const form = await request.formData()
  const scope = form.get("scope")
  const targetId = Number(form.get("target_id"))
  const watching = form.get("watching")
  const validScope = scope === "site" || scope === "category" || scope === "page"
  const validTarget = Number.isSafeInteger(targetId) && targetId > 0
  const validChoice = watching === "true" || watching === "false"
  if (!validScope || !validTarget || !validChoice) {
    return fail(400, { message: "Invalid watch selection." })
  }

  const { siteId } = loadSiteInfo(request.headers)
  try {
    return await watchingSubscriptionSet(
      siteId,
      scope as WatchScope,
      targetId,
      watching === "true",
      { sessionToken, siteId }
    )
  } catch (caught) {
    const error = requireDeepwellError(caught)
    return fail(400, { message: error.message })
  }
}
