import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer, invokeSsrAction } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.server.ts"
)

const { actions: rootActions } = await vite.ssrLoadModule("/src/routes/+page.server.ts")

const reads = [
  { action: "fileList", method: "page_get_files" },
  { action: "history", method: "page_revision_range" },
  { action: "revision", method: "page_revision_get" },
  { action: "parentGet", method: "parent_get_all" },
  { action: "voteGet", method: "vote_list" },
  { action: "score", method: "page_get_score" },
  { action: "lockHistory", method: "page_lock_get_history" }
] as const

type Access = "viewer" | "denied" | "missing"

function buildPageViewReply(access: Access) {
  if (access === "viewer") {
    return {
      type: "found",
      data: { page: { page_id: 101, site_id: 11, slug: "private" } }
    }
  }
  return { type: access === "denied" ? "permissions" : "missing", data: {} }
}

async function readPage(
  action: string,
  method: string,
  access: Access,
  target = { siteId: 11, pageId: 101 },
  params: Record<string, string> = { slug: "private", extra: "" }
) {
  const { siteId, pageId } = target
  const root = !params.slug && !params.extra
  const previousFetch = globalThis.fetch
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body))
    let result
    if (rpc.method === "page_view") {
      assert.deepEqual(rpc.params.route, root ? null : params)
      result = buildPageViewReply(access)
    } else if (rpc.method === method) {
      result = { private_data: "Secret revision / file metadata / lock reason" }
    } else {
      throw new Error(`Unexpected RPC ${rpc.method}`)
    }
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, result }))
  }
  try {
    const selectedActions = root ? rootActions : actions
    return await invokeSsrAction(() =>
      selectedActions[action]({
        request: new Request("http://local.test/private", {
          method: "POST",
          headers: {
            "X-Wikijump-Site-Id": "11",
            "X-Wikijump-Site-Slug": "cobalt",
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            siteId,
            pageId,
            slug: "private",
            revisionNumber: 1,
            limit: 20
          })
        }),
        params,
        cookies: { get: () => (access === "viewer" ? "viewer-session" : undefined) },
        locals: { requestContext: { siteId: 11, page: "private" } }
      })
    )
  } finally {
    globalThis.fetch = previousFetch
  }
}

for (const { action, method } of reads) {
  for (const access of ["denied", "missing"] as const) {
    test(`${action} returns no private data when routed page is ${access}`, async () => {
      const response = await readPage(action, method, access)
      assert.ok(!JSON.stringify(response).includes("Secret revision"))
      assert.equal(response.status, access === "denied" ? 403 : 404)
    })
  }
  test(`${action} retains reads for an authorized routed page`, async () => {
    const response = await readPage(action, method, "viewer")
    assert.equal(
      response.res.private_data,
      "Secret revision / file metadata / lock reason"
    )
  })
  test(`${action} reads the configured default page on the root route`, async () => {
    const response = await readPage(
      action,
      method,
      "viewer",
      { siteId: 11, pageId: 101 },
      {}
    )
    assert.equal(
      response.res.private_data,
      "Secret revision / file metadata / lock reason"
    )
  })
  test(`${action} preserves route extras during page authorization`, async () => {
    const response = await readPage(
      action,
      method,
      "viewer",
      { siteId: 11, pageId: 101 },
      { slug: "private", extra: "noredirect/true" }
    )
    assert.equal(
      response.res.private_data,
      "Secret revision / file metadata / lock reason"
    )
  })
  const foreignTargets =
    action === "lockHistory"
      ? [[11, 102]]
      : [
          [12, 101],
          [11, 102]
        ]
  for (const [siteId, pageId] of foreignTargets) {
    test(`${action} cannot select foreign target ${siteId}:${pageId}`, async () => {
      const response = await readPage(action, method, "viewer", { siteId, pageId })
      assert.ok(!JSON.stringify(response).includes("Secret revision"))
      assert.equal(response.status, 403)
    })
  }
}
