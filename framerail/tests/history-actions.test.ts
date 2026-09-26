import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { historyListAction, historyRevisionAction, historyCompareAction } =
  await vite.ssrLoadModule("/src/lib/server/load/history.ts")

function event(
  payload: unknown,
  params: { slug?: string; extra?: string } = { slug: "home:start" }
) {
  return {
    request: new Request("http://local.test/home:start?/historyList", {
      method: "POST",
      headers: {
        "X-Wikijump-Site-Id": "6000011",
        "X-Wikijump-Site-Slug": "cobalt-company"
      },
      body: JSON.stringify(payload)
    }),
    params,
    cookies: { get: () => "session" },
    locals: {
      requestContext: { siteId: 6000011, page: "home:start", sessionToken: "session" }
    }
  }
}

async function rpc(respond: (method: string) => unknown, action: () => Promise<unknown>) {
  const previous = globalThis.fetch
  const calls: { method: string; params: Record<string, unknown> }[] = []
  globalThis.fetch = async (_input, init) => {
    const request = JSON.parse(String(init?.body))
    calls.push(request)
    return new Response(
      JSON.stringify({ jsonrpc: "2.0", id: request.id, result: respond(request.method) }),
      {
        headers: { "content-type": "application/json" }
      }
    )
  }
  try {
    return { result: await action(), calls }
  } finally {
    globalThis.fetch = previous
  }
}

const found = { type: "found", data: { page: { page_id: 42, site_id: 6000011 } } }
const filters = {
  all: false,
  source: true,
  title: false,
  move: false,
  tags: true,
  meta: false,
  files: true
}

test("paged history binds trusted route identity, filters and maximum page size", async () => {
  const listing = {
    origin: "wikidot",
    page: 2,
    per_page: 200,
    total: 401,
    total_pages: 3,
    available: { wikidot: true, local: false },
    rows: []
  }
  const { result, calls } = await rpc(
    (method) => (method === "page_view" ? found : listing),
    () =>
      historyListAction(
        event({
          siteId: 999,
          pageId: 999,
          origin: "wikidot",
          page: 2,
          per_page: 200,
          filters
        }) as never
      )
  )
  assert.deepEqual(result, { res: listing })
  assert.deepEqual(
    calls.map((call) => call.method),
    ["page_view", "page_history_list"]
  )
  assert.deepEqual(calls[1].params, {
    site_id: 6000011,
    page_id: 42,
    origin: "wikidot",
    page: 2,
    per_page: 200,
    filters
  })
})

test("root history action resolves the root route without an undefined slug", async () => {
  const listing = {
    origin: "wikidot",
    page: 1,
    per_page: 20,
    total: 0,
    total_pages: 0,
    available: { wikidot: false, local: false },
    rows: []
  }
  const { result, calls } = await rpc(
    (method) => (method === "page_view" ? found : listing),
    () => historyListAction(event({ origin: "wikidot" }, {}) as never)
  )
  assert.deepEqual(result, { res: listing })
  assert.equal(calls[0].params.route, null)
})

test("source and comparison use separate read-only RPCs with route identity", async () => {
  const revision = {
    id: 15,
    number: 2,
    source: "<unsafe>",
    rendered_html: null,
    representation: "captured"
  }
  const source = await rpc(
    (method) => (method === "page_view" ? found : revision),
    () =>
      historyRevisionAction(
        event({ origin: "wikidot", number: 2, rendered: false, pageId: 999 }) as never
      )
  )
  assert.deepEqual(source.result, { res: revision })
  assert.deepEqual(source.calls[1].params, {
    site_id: 6000011,
    page_id: 42,
    origin: "wikidot",
    number: 2,
    rendered: false
  })
  const comparison = {
    from: 1,
    to: 2,
    lines: [{ kind: "insert", text: "<unsafe>" }],
    representation: "captured"
  }
  const diff = await rpc(
    (method) => (method === "page_view" ? found : comparison),
    () =>
      historyCompareAction(
        event({ origin: "wikidot", from: 1, to: 2, pageId: 999 }) as never
      )
  )
  assert.deepEqual(diff.result, { res: comparison })
  assert.deepEqual(diff.calls[1].params, {
    site_id: 6000011,
    page_id: 42,
    origin: "wikidot",
    from: 1,
    to: 2
  })
})

test("invalid input and missing or restricted pages never access history RPCs", async () => {
  for (const payload of [
    { origin: "wikidot", page: 0 },
    { origin: "wikidot", per_page: 201 },
    { origin: "wikidot", filters: { ...filters, files: "yes" } },
    { origin: "wikidot", filters: { ...filters, tags: "yes" } },
    { origin: "other" }
  ]) {
    const { result, calls } = await rpc(
      () => {
        throw Error("unexpected RPC")
      },
      () => historyListAction(event(payload) as never)
    )
    assert.equal((result as { status: number }).status, 400)
    assert.equal(calls.length, 0)
  }
  for (const payload of [
    { origin: "local", number: -1 },
    { origin: "local", number: 1.5 }
  ]) {
    const { result, calls } = await rpc(
      () => {
        throw Error("unexpected RPC")
      },
      () => historyRevisionAction(event(payload) as never)
    )
    assert.equal((result as { status: number }).status, 400)
    assert.equal(calls.length, 0)
  }
  for (const response of [
    { type: "permissions", data: {} },
    { type: "missing", data: {} }
  ]) {
    const { result, calls } = await rpc(
      () => response,
      () => historyCompareAction(event({ origin: "wikidot", from: 1, to: 2 }) as never)
    )
    assert.equal(
      (result as { status: number }).status,
      response.type === "permissions" ? 403 : 404
    )
    assert.deepEqual(
      calls.map((call) => call.method),
      ["page_view"]
    )
  }
})
