import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer, invokeSsrAction } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule("/src/routes/[x+2d]/admin/+page.server.ts")

type Actor = "member" | "admin" | "pending" | "guest"

type Rpc = { method: string; params: { site: number; name: string } }

function respondToAdminRpc(rpc: Rpc, actor: Actor, sites: Map<number, string>) {
  if (rpc.method === "session_get") {
    if (actor === "guest") return null
    return { user_id: 42, restricted: actor === "pending" }
  }
  if (rpc.method === "admin_view") {
    return { type: actor === "admin" ? "site_found" : "admin_permissions", data: {} }
  }
  if (rpc.method === "site_update") {
    sites.set(rpc.params.site, rpc.params.name)
    return { site_id: rpc.params.site, name: rpc.params.name }
  }
  throw new Error(`Unexpected RPC ${rpc.method}`)
}

async function editSite(actor: Actor, targetSite = 11) {
  const sites = new Map([
    [11, "Cobalt"],
    [12, "Other site"]
  ])
  const previousFetch = globalThis.fetch
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body))
    const result = respondToAdminRpc(rpc, actor, sites)
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, result }))
  }
  try {
    const response = await invokeSsrAction(() =>
      actions.default({
        request: new Request("http://local.test/-/admin", {
          method: "POST",
          headers: { "X-Wikijump-Site-Id": "11", "X-Wikijump-Site-Slug": "cobalt" },
          body: new URLSearchParams({
            action: "edit",
            siteId: String(targetSite),
            name: "Changed site",
            slug: "cobalt",
            tagline: "Test",
            description: "Site description",
            defaultPage: "start",
            locale: "en",
            layout: "wikidot"
          })
        }),
        cookies: { get: () => (actor === "guest" ? undefined : `${actor}-session`) },
        getClientAddress: () => "127.0.0.1"
      })
    )
    return { response, sites: [...sites.entries()] }
  } finally {
    globalThis.fetch = previousFetch
  }
}

for (const actor of ["member", "pending", "guest"] as const) {
  test(`${actor} cannot update a site through the admin POST action`, async () => {
    const { response, sites } = await editSite(actor)
    assert.deepEqual(sites, [
      [11, "Cobalt"],
      [12, "Other site"]
    ])
    assert.equal(response.status, actor === "guest" ? 401 : 403)
  })
}

test("site administrator can update the routed site", async () => {
  const { response, sites } = await editSite("admin")
  assert.equal(response.res.name, "Changed site")
  assert.deepEqual(sites, [
    [11, "Changed site"],
    [12, "Other site"]
  ])
})

test("admin POST cannot select a site outside the trusted route context", async () => {
  const { response, sites } = await editSite("admin", 12)
  assert.deepEqual(sites, [
    [11, "Cobalt"],
    [12, "Other site"]
  ])
  assert.equal(response.status, 403)
})
