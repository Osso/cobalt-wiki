import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createServer } from "vite"

const vite = await createServer({
  server: { middlewareMode: true, ws: false },
  optimizeDeps: { noDiscovery: true, include: [] },
  logLevel: "error"
})
after(() => vite.close())
const { pageDraftGetAction, pageDraftSaveAction, pageDraftDeleteAction } =
  await vite.ssrLoadModule("/src/lib/server/load/page-draft.ts")

type RpcRequest = {
  jsonrpc: string
  id: number | string
  method: string
  params: unknown
}
type RpcResponse = { result: unknown } | { error: { code: number; message: string } }
const savedDraft = {
  title: "Revised title",
  wikitext: "[[include sandbox]]",
  updated_at: "2026-09-23T10:00:00Z"
}

function event(action: string, payload?: unknown, raw?: string) {
  return {
    request: new Request(`http://local.test/page?/${action}`, {
      method: "POST",
      body:
        raw === undefined
          ? new URLSearchParams(
              payload === undefined ? {} : { payload: JSON.stringify(payload) }
            )
          : new URLSearchParams({ payload: raw })
    }),
    locals: {
      requestContext: { siteId: 6000011, page: "guild:welcome", sessionToken: "trusted" }
    }
  }
}

async function invoke(
  action: (event: never) => Promise<unknown>,
  actionName: string,
  payload?: unknown,
  responder: (rpc: RpcRequest) => RpcResponse = () => ({ result: { draft: savedDraft } }),
  raw?: string
) {
  const previousFetch = globalThis.fetch
  const calls: { request: RpcRequest; headers: Headers }[] = []
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
    const result = await action(event(actionName, payload, raw) as never)
    return { result, calls }
  } finally {
    globalThis.fetch = previousFetch
  }
}

test("get and delete use empty RPC params with trusted headers", async () => {
  const fetched = await invoke(pageDraftGetAction, "draftGet")
  assert.deepEqual(fetched.result, { draft: savedDraft })
  const removed = await invoke(pageDraftDeleteAction, "draftDelete", undefined, () => ({
    result: { deleted: true }
  }))
  assert.deepEqual(removed.result, { deleted: true })
  for (const [call, method] of [
    [fetched.calls[0], "page_draft_get"],
    [removed.calls[0], "page_draft_delete"]
  ] as const) {
    assert.equal(call.request.method, method)
    assert.deepEqual(call.request.params, {})
    assert.equal(call.headers.get("X-Deepwell-Site-Id"), "6000011")
    assert.equal(call.headers.get("X-Deepwell-Page"), "guild:welcome")
    assert.equal(call.headers.get("X-Deepwell-Session-Token"), "trusted")
    assert.equal(call.headers.get("content-type"), "application/json")
  }
})

test("get returns null draft without changing editor state", async () => {
  const { result } = await invoke(pageDraftGetAction, "draftGet", undefined, () => ({
    result: { draft: null }
  }))
  assert.deepEqual(result, { draft: null })
})

test("save forwards exact raw source and title without authority fields", async () => {
  const { result, calls } = await invoke(pageDraftSaveAction, "draftSave", {
    title: "Revised title",
    wikitext: "[[include sandbox]]",
    last_revision_id: 19
  })
  assert.deepEqual(result, { draft: savedDraft })
  assert.equal(calls[0].request.method, "page_draft_save")
  assert.deepEqual(calls[0].request.params, {
    title: "Revised title",
    wikitext: "[[include sandbox]]",
    last_revision_id: 19
  })
})

test("save supports scalar structured updates, including null and false", async () => {
  const structured = {
    ...savedDraft,
    form_values: { rank: 2, approved: false, bio: null, unknown: "keep" }
  }
  const { result, calls } = await invoke(
    pageDraftSaveAction,
    "draftSave",
    {
      title: "Revised title",
      form_updates: { rank: 2, approved: false, bio: null }
    },
    () => ({ result: { draft: structured } })
  )
  assert.deepEqual(result, { draft: structured })
  assert.deepEqual(calls[0].request.params, {
    title: "Revised title",
    form_updates: { rank: 2, approved: false, bio: null }
  })
})

test("malformed, ambiguous and authority-bearing save requests fail before RPC", async () => {
  for (const payload of [
    {},
    { title: "x" },
    { wikitext: "x" },
    { title: "x", wikitext: "x", form_updates: {} },
    { title: "x", wikitext: 3 },
    { title: "x", form_updates: { list: [] } },
    { title: "x", wikitext: "x", last_revision_id: -1 },
    { title: "x", wikitext: "x", last_revision_id: 1.5 },
    { title: "x", wikitext: "x", site_id: 1 },
    { title: "x", wikitext: "x", page: "other" },
    { title: "x", wikitext: "x", user_id: 2 }
  ]) {
    const { result, calls } = await invoke(pageDraftSaveAction, "draftSave", payload)
    assert.equal((result as { status: number }).status, 400, JSON.stringify(payload))
    assert.equal(calls.length, 0)
  }
  const malformed = await invoke(
    pageDraftSaveAction,
    "draftSave",
    undefined,
    undefined,
    "{"
  )
  assert.equal((malformed.result as { status: number }).status, 400)
  assert.equal(malformed.calls.length, 0)
})

test("get and delete reject caller-provided identity before RPC", async () => {
  for (const action of [pageDraftGetAction, pageDraftDeleteAction]) {
    const { result, calls } = await invoke(action, "draftGet", { user_id: 2 })
    assert.equal((result as { status: number }).status, 400)
    assert.equal(calls.length, 0)
  }
})

test("backend failures and malformed responses do not disclose private draft source", async () => {
  for (const action of [pageDraftGetAction, pageDraftSaveAction, pageDraftDeleteAction]) {
    const name = action === pageDraftSaveAction ? "draftSave" : "draftGet"
    const payload =
      action === pageDraftSaveAction
        ? { title: "private-title", wikitext: "private-source" }
        : undefined
    for (const responder of [
      () => ({ error: { code: -32000, message: "private-source" } }),
      () => ({ result: { draft: { title: "private-title" } } })
    ]) {
      const { result } = await invoke(action, name, payload, responder)
      assert.equal((result as { status: number }).status, 500)
      assert.equal(JSON.stringify(result).includes("private-source"), false)
      assert.equal(JSON.stringify(result).includes("private-title"), false)
    }
  }
})
