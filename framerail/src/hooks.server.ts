// Hook that runs on every request, including form actions.

import { storeRequestContext } from "$lib/server/load/request-ctx"
import { Layout } from "$lib/types"
import { loadSiteInfo } from "$lib/server/load/site-info"
import type { Handle } from "@sveltejs/kit"

const HTML5_DOCTYPE = "<!doctype html>"
const WIKIDOT_DOCTYPE =
  '<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN" "http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd">'

export const handle: Handle = async ({ event, resolve }) => {
  const { request, cookies, locals, params } = event

  // Gather common request metadata into a shared context.
  const { siteId } = loadSiteInfo(request.headers)
  const page_slug = params.slug
  const sessionToken = cookies.get("wikijump_token")

  storeRequestContext(locals, sessionToken, siteId, page_slug)

  // The layout is set by the successful page load during resolve.
  let documentStart = ""
  let isInitialChunk = true
  return resolve(event, {
    transformPageChunk: ({ html, done }) => {
      if (locals.documentLayout !== Layout.WIKIDOT || !isInitialChunk) return html

      documentStart += html
      if (!done && documentStart.length < HTML5_DOCTYPE.length) return ""

      isInitialChunk = false
      return documentStart.startsWith(HTML5_DOCTYPE)
        ? WIKIDOT_DOCTYPE + documentStart.slice(HTML5_DOCTYPE.length)
        : documentStart
    }
  })
}
