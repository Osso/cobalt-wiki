import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule("/src/routes/[x+2d]/admin/+page.server.ts")

type Actor = "member" | "admin" | "pending" | "guest"

async function editSite(actor: Actor, targetSite = 11) {
  const sites = new Map([
    [11, "Cobalt"],
    [12, "Other site"]
  ])
  const previousFetch = globalThis.fetch
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body))
    let result
    if (rpc.method === "session_get") {
      result = actor === "guest" ? null : { user_id: 42, restricted: actor === "pending" }
    } else if (rpc.method === "admin_view") {
      result = { type: actor === "admin" ? "site_found" : "admin_permissions", data: {} }
    } else if (rpc.method === "site_update") {
      sites.set(rpc.params.site, rpc.params.name)
      result = { site_id: rpc.params.site, name: rpc.params.name }
    } else {
      throw new Error(`Unexpected RPC ${rpc.method}`)
    }
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, result }))
  }
  try {
    let response
    try {
      response = await actions.default({
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
    } catch (error) {
      if (
        typeof error !== "object" ||
        error === null ||
        !("status" in error) ||
        error.status !== 403
      ) {
        throw error
      }
      response = { status: error.status }
    }
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
