import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { editorPagesAction, editorAttachmentsAction } = await vite.ssrLoadModule(
  "/src/lib/server/load/editor-lookup.ts"
)

type RpcRequest = {
  jsonrpc: string
  id: number | string
  method: string
  params: unknown
}
type RpcResponse = { result: unknown } | { error: { code: number; message: string } }

type RpcCall = { request: RpcRequest; headers: Headers }

async function callAction(
  action: (event: never) => Promise<unknown>,
  fields: Record<string, string>,
  responder: (rpc: RpcRequest) => RpcResponse = () => ({ result: [] })
) {
  const previousFetch = globalThis.fetch
  const calls: RpcCall[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as RpcRequest
    calls.push({ request: rpc, headers: new Headers(init?.headers) })
    return new Response(
      JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...responder(rpc) }),
      {
        headers: { "content-type": "application/json" }
      }
    )
  }
  try {
    const event = {
      request: new Request("http://local.test/wiki:page?/editorPages", {
        method: "POST",
        body: new URLSearchParams(fields)
      }),
      locals: {
        requestContext: {
          siteId: 6000011,
          page: "wiki:page",
          sessionToken: "trusted-session"
        }
      }
    }
    return { result: await action(event as never), calls }
  } finally {
    globalThis.fetch = previousFetch
  }
}

const hit = {
  page_id: 381,
  slug: "docs:guide",
  title: "Guide",
  tags: ["private"],
  snippet: "Private excerpt"
}

test("short or missing queries return validation failure without RPC", async () => {
  const invalidQueries: Record<string, string>[] = [{}, { query: " x " }, { query: "  " }]
  for (const fields of invalidQueries) {
    const { result, calls } = await callAction(editorPagesAction, fields)
    assert.equal((result as { status: number }).status, 400)
    assert.deepEqual((result as { data: unknown }).data, {
      message: "Enter at least two characters"
    })
    assert.equal(calls.length, 0)
  }
})

test("page lookup uses trusted request context, never forged body authority", async () => {
  const { result, calls } = await callAction(
    editorPagesAction,
    { query: "  docs ", site_id: "9", page: "other:page", sessionToken: "forged" },
    () => ({ result: [{ slug: hit.slug, title: hit.title }] })
  )
  assert.deepEqual(result, { pages: [{ slug: "docs:guide", title: "Guide" }] })
  assert.equal(calls.length, 1)
  assert.equal(calls[0].request.method, "editor_pages")
  assert.deepEqual(calls[0].request.params, { query: "  docs " })
  assert.equal(calls[0].headers.get("X-Deepwell-Site-Id"), "6000011")
  assert.equal(calls[0].headers.get("X-Deepwell-Page"), "wiki:page")
  assert.equal(calls[0].headers.get("X-Deepwell-Session-Token"), "trusted-session")
})

test("page lookup backend rejection returns safe failure", async () => {
  const { result, calls } = await callAction(
    editorPagesAction,
    { query: "docs" },
    () => ({
      error: { code: -32000, message: "private backend detail" }
    })
  )
  assert.equal(calls.length, 1)
  assert.equal((result as { status: number }).status, 500)
  assert.deepEqual((result as { data: unknown }).data, {
    message: "Unable to look up pages"
  })
  assert.equal(JSON.stringify(result).includes("private backend detail"), false)
})

test("attachment lookup uses trusted context without body authority", async () => {
  const { result, calls } = await callAction(
    editorAttachmentsAction,
    { site_id: "9", page: "other:page", sessionToken: "forged" },
    () => ({ result: [{ name: "file one.png" }] })
  )
  assert.deepEqual(result, { files: [{ name: "file one.png" }] })
  assert.equal(calls.length, 1)
  assert.equal(calls[0].request.method, "editor_attachments")
  assert.deepEqual(calls[0].request.params, {})
  assert.equal(calls[0].headers.get("X-Deepwell-Site-Id"), "6000011")
  assert.equal(calls[0].headers.get("X-Deepwell-Page"), "wiki:page")
  assert.equal(calls[0].headers.get("X-Deepwell-Session-Token"), "trusted-session")
})

test("denied attachment lookup returns safe failure", async () => {
  const { result, calls } = await callAction(editorAttachmentsAction, {}, () => ({
    error: { code: -32000, message: "private denial reason" }
  }))
  assert.equal(calls.length, 1)
  assert.equal((result as { status: number }).status, 403)
  assert.deepEqual((result as { data: unknown }).data, {
    message: "Unable to load attached files"
  })
  assert.equal(JSON.stringify(result).includes("private denial reason"), false)
})
