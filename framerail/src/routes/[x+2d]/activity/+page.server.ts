import { watchingActivity, watchingChange } from "$lib/server/deepwell/watching"
import { loadSiteInfo } from "$lib/server/load/site-info"
import { error, redirect } from "@sveltejs/kit"

import type { PageServerLoad } from "./$types"

export const load: PageServerLoad = async ({ request, url, cookies }) => {
  const sessionToken = cookies.get("wikijump_token")
  if (!sessionToken) redirect(303, "/-/login?origUrl=%2F-%2Factivity")

  const eventIdParam = url.searchParams.get("event")
  if (eventIdParam !== null) {
    const eventId = Number(eventIdParam)
    if (!Number.isSafeInteger(eventId) || eventId <= 0) error(400, "Invalid change.")
    const { siteId } = loadSiteInfo(request.headers)
    const change = await watchingChange(eventId, { sessionToken, siteId })
    if (!change) error(404, "Change unavailable.")
    return { change, items: [], next_before: null }
  }

  const cursor = url.searchParams.get("before")
  const beforeEventId = cursor === null ? undefined : Number(cursor)
  if (cursor !== null && (!Number.isSafeInteger(beforeEventId) || beforeEventId <= 0)) {
    error(400, "Invalid activity cursor.")
  }
  const { siteId } = loadSiteInfo(request.headers)
  const activity = await watchingActivity(siteId, beforeEventId, 20, {
    sessionToken,
    siteId
  })
  return { ...activity, change: null }
}
