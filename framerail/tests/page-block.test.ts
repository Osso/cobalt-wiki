import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.server.ts"
)
const {
  default: BlockPane,
  loadBlock,
  saveBlock
} = await vite.ssrLoadModule("/src/routes/[slug]/[...extra]/BlockPane.svelte")
const { default: Page } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.svelte"
)
const { render } = await vite.ssrLoadModule("svelte/server")
const { set_app } = await vite.ssrLoadModule(
  "/node_modules/@sveltejs/kit/src/runtime/server/app.js"
)
set_app({ decoders: {} })
const { stringify } = await import(
  new URL("../node_modules/.pnpm/node_modules/devalue/index.js", import.meta.url).href
)
const { pageLayoutState } = await vite.ssrLoadModule("/src/lib/stores.svelte.ts")
const { Layout } = await vite.ssrLoadModule("/src/lib/types.ts")

const data = {
  site: { name: "Cobalt" },
  page: { page_id: 314, slug: "home:start" },
  page_revision: { title: "Start", tags: [] },
  options: {},
  compiled_body_html: "<p>Start</p>",
  internationalization: { options: "Options", "wiki-page-lock": "Lock", cancel: "Cancel" }
}

function event(body: unknown) {
  return {
    request: new Request("http://local.test/home:start", {
      method: "POST",
      body: JSON.stringify(body)
    }),
    getClientAddress: () => "2001:db8::7",
    locals: {
      requestContext: { sessionToken: "session-1", siteId: 42, page: "home:start" }
    }
  }
}

async function withRpc(action: () => Promise<unknown>, denied = false) {
  const oldFetch = globalThis.fetch
  const calls: Array<{
    method: string
    params: Record<string, unknown>
    headers: Headers
  }> = []
  globalThis.fetch = async (_input, init) => {
    const request = JSON.parse(String(init?.body))
    calls.push({
      method: request.method,
      params: request.params,
      headers: new Headers(init?.headers)
    })
    return new Response(
      JSON.stringify({
        jsonrpc: "2.0",
        id: request.id,
        ...(denied
          ? { error: { code: 3106, message: "Permission denied" } }
          : {
              result:
                request.method === "page_block_get"
                  ? { blocked: true, can_manage: false }
                  : null
            })
      }),
      { headers: { "content-type": "application/json" } }
    )
  }
  try {
    return { result: await action(), calls }
  } finally {
    globalThis.fetch = oldFetch
  }
}

test("block route reads current state with request context and no body identity", async () => {
  const { result, calls } = await withRpc(() =>
    actions.blockGet(event({ pageId: 314 }) as never)
  )
  assert.deepEqual(result, { res: { blocked: true, can_manage: false } })
  assert.deepEqual(
    calls.map(({ method, params }) => ({ method, params })),
    [{ method: "page_block_get", params: { page: 314 } }]
  )
  assert.equal(calls[0].headers.get("X-Deepwell-Session-Token"), "session-1")
  assert.equal(calls[0].headers.get("X-Deepwell-Site-Id"), "42")
  assert.equal(calls[0].headers.get("X-Deepwell-Page"), "home:start")
})

test("block route submits both checked and unchecked state with actual client IP", async () => {
  for (const blocked of [true, false]) {
    const { result, calls } = await withRpc(() =>
      actions.blockSet(
        event({ pageId: 314, blocked, userId: 1, ip_address: "spoofed" }) as never
      )
    )
    assert.deepEqual(result, {})
    assert.deepEqual(
      calls.map(({ method, params }) => ({ method, params })),
      [
        {
          method: "page_block_set",
          params: { page: 314, blocked, ip_address: "2001:db8::7" }
        }
      ]
    )
  }
})

test("invalid page identity or state cannot reach backend", async () => {
  for (const [action, payload] of [
    [actions.blockGet, { pageId: "314" }],
    [actions.blockGet, { pageId: 0 }],
    [actions.blockGet, null],
    [actions.blockSet, { pageId: 314, blocked: "false" }],
    [actions.blockSet, { pageId: 314.2, blocked: true }]
  ] as const) {
    const { result, calls } = await withRpc(() => action(event(payload) as never))
    assert.equal((result as { status: number }).status, 400)
    assert.equal(calls.length, 0)
  }
})

test("malformed JSON returns 400 without calling backend", async () => {
  const request = new Request("http://local.test/home:start", {
    method: "POST",
    body: "{not-json"
  })
  const { result, calls } = await withRpc(() =>
    actions.blockSet({ ...event({}), request } as never)
  )
  assert.equal((result as { status: number }).status, 400)
  assert.equal(calls.length, 0)
})

test("permission denial is HTTP 403 for both Block actions", async () => {
  for (const [action, payload] of [
    [actions.blockGet, { pageId: 314 }],
    [actions.blockSet, { pageId: 314, blocked: true }]
  ] as const) {
    const { result } = await withRpc(() => action(event(payload) as never), true)
    assert.equal((result as { status: number }).status, 403)
  }
})

test("native layout retains its Lock action", () => {
  pageLayoutState.current = Layout.WIKIJUMP
  const native = render(Page, { props: { data } }).body
  assert.match(native, /button-lock[^>]*>\s*Lock\s*</)
  assert.doesNotMatch(native, />\s*Block\s*</)
})

test("Block pane starts disabled until current state is loaded", () => {
  const body = render(BlockPane, { props: { data, pagePaneState: "lock" } }).body
  assert.match(body, /<h1[^>]*>Block this page<\/h1>/)
  assert.match(body, /Loading page Block state/)
  assert.match(body, /<input[^>]*type="checkbox"[^>]*disabled/)
  assert.match(body, /<button[^>]*type="submit"[^>]*disabled/)
  assert.match(body, /<button[^>]*type="button"[^>]*>Cancel<\/button>/)
})

test("Block client reads baseline and submits only on Save, preserving false", async () => {
  const calls: Array<{ url: string; body: unknown }> = []
  const fetcher = async (url: string, init: RequestInit) => {
    calls.push({ url, body: JSON.parse(String(init.body)) })
    return {
      text: async () =>
        JSON.stringify({
          type: "success",
          data: stringify(
            url === "?/blockGet" ? { res: { blocked: true, can_manage: true } } : {}
          )
        })
    }
  }
  const baseline = await loadBlock(fetcher, 314)
  assert.deepEqual(baseline, { blocked: true, can_manage: true })
  assert.deepEqual(calls, [{ url: "?/blockGet", body: { pageId: 314 } }])
  await saveBlock(fetcher, 314, false)
  assert.deepEqual(calls[1], { url: "?/blockSet", body: { pageId: 314, blocked: false } })
})

test("Block client surfaces failed reads and writes", async () => {
  const fetcher = async () => ({
    text: async () =>
      JSON.stringify({
        type: "failure",
        status: 403,
        data: stringify({ message: "Permission denied" })
      })
  })
  await assert.rejects(loadBlock(fetcher, 314), /Permission denied/)
  await assert.rejects(saveBlock(fetcher, 314, true), /Permission denied/)
})
