import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.server.ts"
)

for (const [code, status] of [
  [3106, 403],
  [1200, 500]
]) {
  test(`page edit maps backend error ${code} to HTTP ${status}`, async () => {
    const previousFetch = globalThis.fetch
    globalThis.fetch = async (_input, init) => {
      const rpc = JSON.parse(String(init?.body))
      const response =
        rpc.method === "session_get"
          ? { result: { user_id: 42, restricted: false } }
          : rpc.method === "page_edit"
            ? { error: { code, message: "Edit failed", data: null } }
            : (() => {
                throw new Error(`Unexpected RPC ${rpc.method}`)
              })()
      return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...response }))
    }
    try {
      const response = await actions.edit({
        request: new Request("http://local.test/page", {
          method: "POST",
          headers: { "X-Wikijump-Site-Id": "11", "X-Wikijump-Site-Slug": "cobalt" },
          body: new URLSearchParams({
            siteId: "11",
            pageId: "101",
            lastRevisionId: "501",
            title: "Page",
            altTitle: "",
            wikitext: "Replacement page",
            tags: "",
            comments: "Edit"
          })
        }),
        params: { slug: "page", extra: "" },
        cookies: { get: () => "member-session" },
        getClientAddress: () => "127.0.0.1",
        locals: {
          requestContext: { siteId: 11, page: "page", sessionToken: "member-session" }
        }
      })
      assert.equal(response.status, status)
      assert.equal(response.data.code, code)
    } finally {
      globalThis.fetch = previousFetch
    }
  })
}
