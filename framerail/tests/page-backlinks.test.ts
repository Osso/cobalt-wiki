import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { pageBacklinksAction } = await vite.ssrLoadModule(
  "/src/lib/server/load/page-backlinks.ts"
)

async function readBacklinks(view: unknown, root = false) {
  const original = globalThis.fetch
  const calls: { method: string; params: Record<string, unknown> }[] = []
  const backlinks = {
    links: [{ page_id: 8, slug: "notes:example", title: "Example <safe>" }],
    inclusions: [{ page_id: 9, slug: "notes:included", title: "Included" }]
  }
  globalThis.fetch = async (_input, init) => {
    const request = JSON.parse(String(init?.body))
    calls.push(request)
    return new Response(
      JSON.stringify({
        jsonrpc: "2.0",
        id: request.id,
        result: request.method === "page_view" ? view : backlinks
      }),
      { headers: { "content-type": "application/json" } }
    )
  }
  try {
    const result = await pageBacklinksAction({
      request: new Request("http://local.test/home:start?/backlinks", {
        method: "POST",
        headers: {
          "X-Wikijump-Site-Id": "6000011",
          "X-Wikijump-Site-Slug": "cobalt-company"
        },
        body: JSON.stringify({ site_id: 999, page_id: 999 })
      }),
      params: root ? {} : { slug: "home:start" },
      cookies: { get: () => "session" },
      locals: {
        requestContext: { siteId: 6000011, page: "home:start", sessionToken: "session" }
      }
    })
    return { result, calls, backlinks }
  } finally {
    globalThis.fetch = original
  }
}

const found = { type: "found", data: { page: { page_id: 42, site_id: 6000011 } } }

test("backlinks bind trusted route identity, not submitted site or page", async () => {
  const { result, calls, backlinks } = await readBacklinks(found)
  assert.deepEqual(result, { res: backlinks })
  assert.deepEqual(
    calls.map((call) => call.method),
    ["page_view", "page_backlinks"]
  )
  assert.deepEqual(calls[1].params, { site_id: 6000011, page_id: 42 })
})

test("root backlinks resolve the current root page", async () => {
  const { result, calls, backlinks } = await readBacklinks(found, true)
  assert.deepEqual(result, { res: backlinks })
  assert.equal(calls[0].params.route, null)
})

test("denied target does not request backlink identities", async () => {
  const { result, calls } = await readBacklinks({ type: "permissions" })
  assert.equal(result.status, 403)
  assert.equal(calls.length, 1)
})

test("missing target does not request backlink identities", async () => {
  const { result, calls } = await readBacklinks({ type: "missing" })
  assert.equal(result.status, 404)
  assert.equal(calls.length, 1)
})
