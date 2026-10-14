import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer, invokeSsrAction } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.server.ts"
)

const panes = [
  "fileList",
  "history",
  "revision",
  "importedHistory",
  "importedRevision",
  "historyList",
  "historyRevision",
  "historyCompare",
  "parentGet",
  "voteGet",
  "score",
  "lockHistory",
  "backlinks"
] as const
const privateResult = { private_data: "Owned private-page fixture" }

type Rpc = {
  id: string | number
  method: string
  params: { locales?: string[] }
}

function pageReply(rpc: Rpc, allowed: boolean) {
  const locales = rpc.params.locales ?? []
  let code = locales.length ? 0 : 5005
  for (const locale of locales) {
    try {
      new Intl.Locale(locale)
    } catch {
      code = 5000
    }
  }
  if (code) {
    return {
      error: {
        code,
        message:
          code === 5005 ? "No locales were specified in the request" : "Invalid locale",
        data: { call_trace: "Owned private-page fixture" }
      }
    }
  }
  return {
    result: allowed
      ? { type: "found", data: { page: { page_id: 101, site_id: 11 } } }
      : { type: "permissions", data: {} }
  }
}

async function readPane(action: string, allowed: boolean, language?: string) {
  const previousFetch = globalThis.fetch
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as Rpc
    const reply =
      rpc.method === "page_view" ? pageReply(rpc, allowed) : { result: privateResult }
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...reply }))
  }
  const headers = new Headers({
    "X-Wikijump-Site-Id": "11",
    "X-Wikijump-Site-Slug": "cobalt",
    "Content-Type": "application/json"
  })
  if (language !== undefined) headers.set("Accept-Language", language)
  try {
    return await invokeSsrAction(() =>
      actions[action]({
        request: new Request("http://local.test/player:owned", {
          method: "POST",
          headers,
          body: JSON.stringify({
            siteId: 11,
            pageId: 101,
            deleted: false,
            slug: "player:owned",
            revisionNumber: 1,
            sourceRevisionNumber: 1,
            origin: "local",
            number: 1,
            from: 0,
            to: 1,
            limit: 20
          })
        }),
        params: { slug: "player:owned", extra: "" },
        cookies: { get: () => (allowed ? "owned-member-session" : undefined) },
        locals: { requestContext: { siteId: 11, page: "player:owned" } }
      })
    )
  } finally {
    globalThis.fetch = previousFetch
  }
}

for (const language of [undefined, "*"]) {
  for (const pane of panes) {
    test(`${pane} denies private reads with ${language ?? "no"} language header`, async () => {
      const response = await readPane(pane, false, language)
      assert.equal(response.status, 403)
      assert.ok(!JSON.stringify(response).includes("Owned private-page fixture"))
    })
    test(`${pane} retains member reads with ${language ?? "no"} language header`, async () => {
      const response = await readPane(pane, true, language)
      assert.deepEqual(response.res, privateResult)
    })
  }
}
