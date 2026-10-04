import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.server.ts"
)

async function submitVote(action: "voteCast" | "voteCancel", restricted: boolean) {
  const votes = new Map([
    ["42:101", 1],
    ["99:101", 1]
  ])
  const originalFetch = globalThis.fetch
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body))
    let result
    if (rpc.method === "session_get") {
      result = { user_id: 42, session_token: "pending-token", restricted }
    } else if (rpc.method === "vote_set") {
      votes.set(`${rpc.params.user_id}:${rpc.params.page_id}`, rpc.params.value)
      result = { user_id: rpc.params.user_id, page_id: rpc.params.page_id }
    } else if (rpc.method === "vote_remove") {
      votes.delete(`${rpc.params.user_id}:${rpc.params.page_id}`)
      result = { user_id: rpc.params.user_id, page_id: rpc.params.page_id }
    } else {
      throw new Error(`Unexpected RPC ${rpc.method}`)
    }
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, result }))
  }
  try {
    let response
    try {
      response = await actions[action]({
        request: new Request("http://local.test/page", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            siteId: 1,
            pageId: 101,
            value: -1,
            user_id: 99,
            userId: 99
          })
        }),
        cookies: { get: () => "pending-token" }
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
    return { response, votes: [...votes.entries()] }
  } finally {
    globalThis.fetch = originalFetch
  }
}

for (const action of ["voteCast", "voteCancel"] as const) {
  test(`pending MFA cannot ${action} through the public page action`, async () => {
    const { response, votes } = await submitVote(action, true)
    assert.deepEqual(votes, [
      ["42:101", 1],
      ["99:101", 1]
    ])
    assert.equal(response.status, 403)
  })
  test(`authenticated ${action} ignores forged body identity and preserves another user's vote`, async () => {
    const { response, votes } = await submitVote(action, false)
    assert.ok(response.res)
    assert.deepEqual(
      votes,
      action === "voteCast"
        ? [
            ["42:101", -1],
            ["99:101", 1]
          ]
        : [["99:101", 1]]
    )
  })
}
