import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createServer } from "vite"

const vite = await createServer({ server: { middlewareMode: true }, logLevel: "error" })
after(() => vite.close())
const { pagePreviewAction } = await vite.ssrLoadModule(
  "/src/lib/server/load/page-preview.ts"
)

type RpcRequest = {
  jsonrpc: string
  id: number | string
  method: string
  params: unknown
}

function event(payload: unknown) {
  return {
    request: new Request("http://local.test/page?/preview", {
      method: "POST",
      body: JSON.stringify(payload)
    }),
    locals: {
      requestContext: { siteId: 6000011, page: "guild:welcome", sessionToken: "trusted" }
    }
  }
}

async function callPreview(
  payload: unknown,
  responder: (rpc: RpcRequest) => unknown = () => ({
    result: { html: "<p>Rendered preview</p>" }
  })
) {
  const previousFetch = globalThis.fetch
  const calls: { request: RpcRequest; headers: Headers }[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as RpcRequest
    calls.push({ request: rpc, headers: new Headers(init?.headers) })
    return new Response(
      JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...responder(rpc) }),
      { headers: { "content-type": "application/json" } }
    )
  }
  try {
    return { result: await pagePreviewAction(event(payload) as never), calls }
  } finally {
    globalThis.fetch = previousFetch
  }
}

test("raw preview forwards only edited fields and trusted context in JSON-RPC", async () => {
  const { result, calls } = await callPreview(
    {
      title: "Welcome",
      alt_title: null,
      tags: ["guild"],
      wikitext: "[[include sandbox]]",
      last_revision_id: 19
    },
    () => ({ result: { html: "<p>Rendered preview</p>" } })
  )
  assert.deepEqual(result, { html: "<p>Rendered preview</p>" })
  assert.equal(calls.length, 1)
  assert.deepEqual(calls[0].request, {
    jsonrpc: "2.0",
    id: calls[0].request.id,
    method: "page_preview",
    params: {
      title: "Welcome",
      alt_title: null,
      tags: ["guild"],
      wikitext: "[[include sandbox]]",
      last_revision_id: 19
    }
  })
  assert.equal(calls[0].headers.get("X-Deepwell-Site-Id"), "6000011")
  assert.equal(calls[0].headers.get("X-Deepwell-Page"), "guild:welcome")
  assert.equal(calls[0].headers.get("X-Deepwell-Session-Token"), "trusted")
})

test("structured preview accepts scalar updates including null and false", async () => {
  const { result, calls } = await callPreview(
    { form_updates: { rank: 2, approved: false, bio: null }, tags: [] },
    () => ({ result: { html: "<em>Draft</em>" } })
  )
  assert.deepEqual(result, { html: "<em>Draft</em>" })
  assert.deepEqual(calls[0].request.params, {
    form_updates: { rank: 2, approved: false, bio: null },
    tags: []
  })
})

test("malformed, ambiguous and authority-bearing requests fail before RPC", async () => {
  for (const payload of [
    {},
    { wikitext: "x", form_updates: {} },
    { wikitext: 3 },
    { form_updates: { list: [] } },
    { tags: "guild", wikitext: "x" },
    { alt_title: 4, wikitext: "x" },
    { last_revision_id: -1, wikitext: "x" },
    { last_revision_id: 1.5, wikitext: "x" },
    { site_id: 123, wikitext: "x" },
    { page: "other", wikitext: "x" },
    { user_id: 123, wikitext: "x" }
  ]) {
    const { result, calls } = await callPreview(payload)
    assert.equal((result as { status: number }).status, 400, JSON.stringify(payload))
    assert.equal(calls.length, 0, JSON.stringify(payload))
  }
})

test("backend rejection returns popup-safe failure without private source", async () => {
  const { result, calls } = await callPreview({ wikitext: "private-source" }, () => ({
    error: {
      code: -32000,
      message: "Rejected private-source",
      data: { call_trace: "private-source" }
    }
  }))
  assert.equal(calls.length, 1)
  assert.equal((result as { status: number }).status, 500)
  assert.equal(JSON.stringify(result).includes("private-source"), false)
  assert.equal(typeof (result as { data: { message: unknown } }).data.message, "string")
})
