import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createServer } from "vite"

const vite = await createServer({ server: { middlewareMode: true }, logLevel: "error" })
after(() => vite.close())
const { actions } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.server.ts"
)
const importedHistoryAction = actions.importedHistory
const importedRevisionAction = actions.importedRevision

type RpcRequest = { method: string; params: Record<string, unknown>; id: string | number }

function routedEvent(payload: unknown, slug = "home:start", sessionToken?: string) {
  const request = new Request("http://local.test/home:start?/importedHistory", {
    method: "POST",
    headers: {
      "X-Wikijump-Site-Id": "6000011",
      "X-Wikijump-Site-Slug": "cobalt-company"
    },
    body: JSON.stringify(payload)
  })
  return {
    request,
    params: { slug },
    cookies: { get: () => sessionToken },
    locals: { requestContext: { siteId: 6000011, page: slug, sessionToken } }
  }
}

function withRpc(
  responder: (request: RpcRequest) => unknown,
  action: () => Promise<unknown>
): Promise<{ response: unknown; calls: RpcRequest[]; headers: Headers[] }> {
  const previousFetch = globalThis.fetch
  const calls: RpcRequest[] = []
  const headers: Headers[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as RpcRequest
    calls.push(rpc)
    headers.push(new Headers(init?.headers))
    const output = responder(rpc)
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, result: output }), {
      headers: { "content-type": "application/json" }
    })
  }
  return action()
    .then((response) => ({ response, calls, headers }))
    .finally(() => {
      globalThis.fetch = previousFetch
    })
}

const pageView = {
  type: "found",
  data: { page: { page_id: 42, site_id: 6000011 }, page_revision: { revision_id: 19 } }
}

// These tests check observable JSON-RPC boundary behavior, not source structure.
test("list uses routed page identity, trusted context and exclusive cursor", async () => {
  const result = await withRpc(
    (rpc) =>
      rpc.method === "page_view"
        ? pageView
        : [
            {
              source_revision_id: 103,
              source_revision_number: 3,
              representation: "display-decoded-not-byte-exact"
            }
          ],
    () =>
      importedHistoryAction(
        routedEvent(
          { siteId: 999, pageId: 999, beforeRevision: 4, limit: 50 },
          "home:start",
          "session"
        ) as never
      )
  )
  assert.deepEqual(result.response, {
    res: [
      {
        source_revision_id: 103,
        source_revision_number: 3,
        representation: "display-decoded-not-byte-exact"
      }
    ]
  })
  assert.equal(result.calls[0].method, "page_view")
  assert.deepEqual(result.calls[1], {
    jsonrpc: "2.0",
    id: result.calls[1].id,
    method: "page_imported_history",
    params: { site_id: 6000011, page_id: 42, before_revision: 4, limit: 50 }
  })
  assert.equal(result.headers[1].get("X-Deepwell-Site-Id"), "6000011")
  assert.equal(result.headers[1].get("X-Deepwell-Page"), "home:start")
  assert.equal(result.headers[1].get("X-Deepwell-Session-Token"), "session")
})

test("source fetch returns imported wikitext without native rollback", async () => {
  const source = {
    metadata: {
      source_revision_number: 0,
      representation: "display-decoded-not-byte-exact"
    },
    wikitext: "old source"
  }
  const result = await withRpc(
    (rpc) => (rpc.method === "page_view" ? pageView : source),
    () =>
      importedRevisionAction(
        routedEvent({ pageId: 999, sourceRevisionNumber: 0 }) as never
      )
  )
  assert.deepEqual(result.response, { res: source })
  assert.equal(result.calls[1].method, "page_imported_revision")
  assert.deepEqual(result.calls[1].params, {
    site_id: 6000011,
    page_id: 42,
    source_revision_number: 0
  })
  assert.equal(result.headers[1].get("X-Deepwell-Session-Token"), null)
})

test("permission denied and missing pages cannot call history endpoints", async () => {
  for (const view of [
    { type: "permissions", data: { banned: false } },
    { type: "missing", data: {} }
  ]) {
    const result = await withRpc(
      () => view,
      () => importedHistoryAction(routedEvent({ limit: 50 }) as never)
    )
    assert.equal(
      (result.response as { status: number }).status,
      view.type === "permissions" ? 403 : 404
    )
    assert.deepEqual(
      result.calls.map(({ method }) => method),
      ["page_view"]
    )
  }
})

test("invalid list cursor, limit and source number fail before RPC", async () => {
  for (const payload of [
    { limit: 0 },
    { limit: 101 },
    { beforeRevision: -1 },
    { beforeRevision: 1.5 },
    { limit: "50" }
  ]) {
    const result = await withRpc(
      () => {
        throw Error("unexpected RPC")
      },
      () => importedHistoryAction(routedEvent(payload) as never)
    )
    assert.equal((result.response as { status: number }).status, 400)
    assert.equal(result.calls.length, 0)
  }
  for (const value of [-1, 1.5, "0", null]) {
    const result = await withRpc(
      () => {
        throw Error("unexpected RPC")
      },
      () => importedRevisionAction(routedEvent({ sourceRevisionNumber: value }) as never)
    )
    assert.equal((result.response as { status: number }).status, 400)
    assert.equal(result.calls.length, 0)
  }
})
