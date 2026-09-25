import { watchingUnsubscribe } from "$lib/server/deepwell/watching"
import { requireDeepwellError } from "$lib/deepwell-errors"

import type { PageServerLoad } from "./$types"

export const load: PageServerLoad = async ({ url }) => {
  const token = url.searchParams.get("token")
  if (!token) return { unsubscribed: false }
  try {
    return await watchingUnsubscribe(token)
  } catch (caught) {
    requireDeepwellError(caught)
    return { unsubscribed: false }
  }
}
