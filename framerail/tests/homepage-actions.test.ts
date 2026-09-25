import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { handle } = await vite.ssrLoadModule("/src/hooks.server.ts")
const root = await vite.ssrLoadModule("/src/routes/+page.server.ts")
const slug = await vite.ssrLoadModule("/src/routes/[slug]/[...extra]/+page.server.ts")

const siteId = "6000011"
const homepage = "guild:welcome"

async function requestPermission(
  path: string,
  routeId: string,
  params: Record<string, string>
) {
  const previousFetch = globalThis.fetch
  const calls: { method: string; page: string | null }[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body))
    const page = new Headers(init?.headers).get("X-Deepwell-Page")
    calls.push({ method: rpc.method, page })
    const response =
      rpc.method === "preload_view"
        ? { result: { site: { default_page: homepage }, user_session: null } }
        : page === (path === "/" ? homepage : "roster")
          ? { result: { can_edit: false } }
          : { error: { code: 1001, message: "Page reference not present" } }
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...response }))
  }
  const request = new Request(`http://local.test${path}?/editPermission`, {
    method: "POST",
    headers: { "X-Wikijump-Site-Id": siteId, "X-Wikijump-Site-Slug": "cobalt-company" },
    body: new URLSearchParams()
  })
  const event = {
    request,
    route: { id: routeId },
    params,
    cookies: { get: () => undefined },
    locals: {}
  }
  try {
    const response = await handle({
      event,
      resolve: async (handledEvent: typeof event) => {
        const action =
          path === "/" ? root.actions.editPermission : slug.actions.editPermission
        const result = await action(handledEvent)
        return Response.json(result)
      }
    })
    return { result: await response.json(), calls, locals: event.locals }
  } finally {
    globalThis.fetch = previousFetch
  }
}

test("root edit permission uses configured homepage and denies anonymous edits", async () => {
  const { result, calls } = await requestPermission("/", "/", {})
  assert.deepEqual(result, { res: { can_edit: false } })
  assert.deepEqual(calls, [
    { method: "preload_view", page: null },
    { method: "page_edit_permission", page: homepage }
  ])
})

test("explicit page uses its route slug without loading homepage", async () => {
  const { result, calls } = await requestPermission("/roster", "/[slug]/[...extra]", {
    slug: "roster"
  })
  assert.deepEqual(result, { res: { can_edit: false } })
  assert.deepEqual(calls, [{ method: "page_edit_permission", page: "roster" }])
})

test("non-page routes do not acquire homepage context", async () => {
  const request = new Request("http://local.test/-/admin", {
    method: "POST",
    headers: { "X-Wikijump-Site-Id": siteId, "X-Wikijump-Site-Slug": "cobalt-company" }
  })
  const event = {
    request,
    route: { id: "/[x+2d]/admin" },
    params: {},
    cookies: { get: () => undefined },
    locals: {}
  }
  const response = await handle({
    event,
    resolve: async (handledEvent: typeof event) => Response.json(handledEvent.locals)
  })
  assert.deepEqual(await response.json(), { requestContext: { siteId: 6000011 } })
})
