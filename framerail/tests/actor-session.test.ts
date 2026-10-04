import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer, invokeSsrAction } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.server.ts"
)

type Authentication = "pending" | "authenticated" | "expired" | "missing-cookie"

type RoutedPage = "viewable" | "denied"

type VoteRpc = {
  method: string
  params: { user_id?: number; page_id: number; value: number }
}

function buildSessionReply(authentication: Authentication) {
  if (authentication === "expired" || authentication === "missing-cookie") return null
  return {
    user_id: 42,
    session_token: "session-token",
    restricted: authentication === "pending"
  }
}

function buildPageViewReply(routedPage: RoutedPage) {
  if (routedPage === "denied") return { type: "permissions", data: {} }
  return { type: "found", data: { page: { page_id: 101, site_id: 1, slug: "page" } } }
}

function respondToVoteRpc(
  rpc: VoteRpc,
  authentication: Authentication,
  routedPage: RoutedPage,
  votes: Map<string, number>
) {
  if (rpc.method === "session_get") return { result: buildSessionReply(authentication) }
  if (rpc.method === "page_view") return { result: buildPageViewReply(routedPage) }
  const isVoteMutation = rpc.method === "vote_set" || rpc.method === "vote_remove"
  if (isVoteMutation && rpc.params.user_id === undefined) {
    return { error: { code: -32602, message: "Invalid params" } }
  }
  const voteKey = `${rpc.params.user_id}:${rpc.params.page_id}`
  if (rpc.method === "vote_set") {
    votes.set(voteKey, rpc.params.value)
    return { result: { user_id: rpc.params.user_id, page_id: rpc.params.page_id } }
  }
  if (rpc.method === "vote_remove") {
    votes.delete(voteKey)
    return { result: { user_id: rpc.params.user_id, page_id: rpc.params.page_id } }
  }
  throw new Error(`Unexpected RPC ${rpc.method}`)
}

async function submitVote(
  action: "voteCast" | "voteCancel",
  authentication: Authentication,
  { pageId = 101, routedPage = "viewable" as RoutedPage } = {}
) {
  const votes = new Map([
    ["42:101", 1],
    ["99:101", 1]
  ])
  const originalFetch = globalThis.fetch
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body))
    const reply = respondToVoteRpc(rpc, authentication, routedPage, votes)
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...reply }))
  }
  try {
    const response = await invokeSsrAction(() =>
      actions[action]({
        request: new Request("http://local.test/page", {
          method: "POST",
          headers: {
            "X-Wikijump-Site-Id": "1",
            "X-Wikijump-Site-Slug": "cobalt",
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            siteId: 1,
            pageId,
            value: -1,
            user_id: 99,
            userId: 99
          })
        }),
        params: { slug: "page", extra: "" },
        cookies: {
          get: () => (authentication === "missing-cookie" ? undefined : "session-token")
        }
      })
    )
    return { response, votes: [...votes.entries()] }
  } finally {
    globalThis.fetch = originalFetch
  }
}

for (const action of ["voteCast", "voteCancel"] as const) {
  for (const authentication of ["expired", "missing-cookie"] as const) {
    test(`${authentication} authentication cannot ${action} and returns 401`, async () => {
      const { response, votes } = await submitVote(action, authentication)
      assert.deepEqual(votes, [
        ["42:101", 1],
        ["99:101", 1]
      ])
      assert.equal(response.status, 401)
    })
  }
  test(`pending MFA cannot ${action} through the public page action`, async () => {
    const { response, votes } = await submitVote(action, "pending")
    assert.deepEqual(votes, [
      ["42:101", 1],
      ["99:101", 1]
    ])
    assert.equal(response.status, 403)
  })
  test(`authenticated ${action} ignores forged body identity and preserves another user's vote`, async () => {
    const { response, votes } = await submitVote(action, "authenticated")
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
  test(`authenticated ${action} cannot target a page other than the routed one`, async () => {
    const { response, votes } = await submitVote(action, "authenticated", { pageId: 202 })
    assert.deepEqual(votes, [
      ["42:101", 1],
      ["99:101", 1]
    ])
    assert.equal(response.status, 403)
  })
  test(`authenticated ${action} cannot vote on a page it cannot view`, async () => {
    const { response, votes } = await submitVote(action, "authenticated", {
      routedPage: "denied"
    })
    assert.deepEqual(votes, [
      ["42:101", 1],
      ["99:101", 1]
    ])
    assert.equal(response.status, 403)
  })
}
