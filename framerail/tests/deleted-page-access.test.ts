import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.server.ts"
)

async function deletedLookup(siteId = 11, slug = "restore:deleted", route = slug) {
  const previousFetch = globalThis.fetch
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body))
    assert.equal(rpc.method, "page_get_deleted")
    const headers = new Headers(init?.headers)
    // Editable body targets deliberately return real metadata unless trusted
    // context reaches the RPC. This preserves the original identity-bypass boundary.
    const missingContext = !headers.has("X-Deepwell-Site-Id")
    const authorized =
      missingContext ||
      (headers.get("X-Deepwell-Session-Token") === "editor-session" &&
        headers.get("X-Deepwell-Site-Id") === String(rpc.params.site_id) &&
        headers.get("X-Deepwell-Page") === rpc.params.slug)
    return new Response(
      JSON.stringify({
        jsonrpc: "2.0",
        id: rpc.id,
        ...(authorized
          ? {
              result: [
                {
                  page_id: 101,
                  title: "Deleted secret title",
                  ingress: {
                    site: headers.get("X-Deepwell-Site-Id"),
                    page: headers.get("X-Deepwell-Page"),
                    session: headers.get("X-Deepwell-Session-Token")
                  }
                }
              ]
            }
          : { error: { code: 3106, message: "Permission denied" } })
      })
    )
  }
  try {
    return await actions.deletedGet({
      request: new Request(`http://local.test/${route}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ siteId, slug })
      }),
      params: { slug: route || undefined, extra: "" },
      locals: {
        requestContext: {
          siteId: 11,
          page: "restore:deleted",
          sessionToken: "editor-session"
        }
      }
    })
  } finally {
    globalThis.fetch = previousFetch
  }
}

test("deletedGet forwards trusted context for authorized restore lookup", async () => {
  const response = await deletedLookup()
  assert.equal(response.res[0].title, "Deleted secret title")
  assert.deepEqual(response.res[0].ingress, {
    site: "11",
    page: "restore:deleted",
    session: "editor-session"
  })
})
for (const [siteId, slug] of [
  [12, "restore:deleted"],
  [11, "other:deleted"]
] as const) {
  test(`deletedGet rejects editable foreign body target ${siteId}:${slug}`, async () => {
    const response = await deletedLookup(siteId, slug)
    assert.equal(response.data.code, 3106)
    assert.ok(!JSON.stringify(response).includes("Deleted secret title"))
  })
}
test("root deletedGet uses configured default page from hooks context", async () => {
  const response = await deletedLookup(11, "restore:deleted", "")
  assert.equal(response.res[0].page_id, 101)
})
