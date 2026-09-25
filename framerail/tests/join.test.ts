import assert from "node:assert/strict"
import { after, test } from "node:test"
import type { loadJoinPage } from "../src/lib/server/load/join"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const headers = { "X-Wikijump-Site-Id": "6000011", "X-Wikijump-Site-Slug": "cobalt" }
const { load, actions } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/join/+page.server.ts"
)
const { default: JoinPage } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/join/+page.svelte"
)
const { render } = await vite.ssrLoadModule("svelte/server")

type Status = {
  is_member: boolean
  application: { message: string; created_at: string } | null
}
type Call = {
  method: string
  params: Record<string, unknown>
  headers: Record<string, string>
}
type JoinData = Awaited<ReturnType<typeof loadJoinPage>>
type ActionResult = {
  submitted?: boolean
  message?: string
  applicationMessage?: string
  status?: number
  data?: { message: string; submitted?: boolean }
}
async function withBackend<T>(status: Status, run: () => Promise<T>, errorCode?: number) {
  const previous = globalThis.fetch
  const calls: Call[] = []
  globalThis.fetch = async (_url, init) => {
    const rpc = JSON.parse(String(init?.body))
    calls.push({ ...rpc, headers: init?.headers })
    let result: unknown
    if (rpc.method === "translate") result = {}
    else if (rpc.method === "page_view") {
      result = { type: "missing", data: { compiled_top_bar_html: "<nav>Top</nav>" } }
    } else if (rpc.method === "member_application_get") result = status
    else if (rpc.method === "member_application_submit") result = {}
    else throw new Error(`Unexpected RPC ${rpc.method}`)
    const response =
      errorCode && rpc.method.startsWith("member_application_")
        ? { error: { code: errorCode, message: "Request refused" } }
        : { result }
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...response }))
  }
  try {
    return { value: await run(), calls }
  } finally {
    globalThis.fetch = previous
  }
}
const guest = { is_member: false, application: null }
function loadJoin(token: string | null = "guest-session") {
  return (load as (event: unknown) => Promise<JoinData>)(event(token))
}
function submitJoin(token: string | null, message: string) {
  return (actions.default as (event: unknown) => Promise<ActionResult>)(
    event(token, message)
  )
}
function event(token: string | null = "guest-session", message?: string) {
  return {
    request: new Request("http://local.test/-/join", {
      headers,
      ...(message === undefined
        ? {}
        : {
            method: "POST",
            body: new URLSearchParams({ message, user_id: "999", site_id: "666" })
          })
    }),
    cookies: { get: () => token ?? undefined },
    getClientAddress: () => "203.0.113.9",
    parent: async () => ({
      locales: ["en"],
      user_session: token ? { user: { user_id: 42 }, session: {} } : null
    })
  }
}

test("signed-out visitors sign in or create an account, not apply anonymously", async () => {
  const { value, calls } = await withBackend(guest, () => loadJoin(null))
  const html = render(JoinPage, { props: { data: value } }).body
  assert.match(html, /Sign in/)
  assert.match(html, /Create an account/)
  assert.match(html, /guests/)
  assert.ok(!calls.some((call) => call.method.startsWith("member_application_")))
  const refused = await withBackend(guest, () => submitJoin(null, "Let me join"))
  assert.equal(refused.value.status, 401)
  assert.equal(refused.calls.length, 0)
})

test("a guest sees a message form; pending and member states cannot submit", async () => {
  for (const [status, text] of [
    [guest, "Why would you like to join?"],
    [
      {
        is_member: false,
        application: { message: "I enjoy this wiki", created_at: "2026-09-25T00:00:00Z" }
      },
      "awaiting administrator review"
    ],
    [{ is_member: true, application: null }, "already a member"]
  ] as const) {
    const { value } = await withBackend(status, () => loadJoin())
    const html = render(JoinPage, { props: { data: value } }).body
    assert.ok(html.includes(text))
    assert.equal(html.includes("<textarea"), status === guest)
  }
})

test("application submits only message and IP with session/site headers", async () => {
  const { value, calls } = await withBackend(guest, () =>
    submitJoin("guest-session", "  I'd like to contribute.  ")
  )
  assert.equal(value.submitted, true)
  assert.equal(calls.length, 1)
  assert.deepEqual(calls[0].params, {
    message: "I'd like to contribute.",
    ip_address: "203.0.113.9"
  })
  assert.equal(calls[0].headers["X-Deepwell-Session-Token"], "guest-session")
  assert.equal(calls[0].headers["X-Deepwell-Site-Id"], "6000011")
})

test("empty and oversized messages fail before RPC; backend refusals remain failures", async () => {
  for (const message of ["  ", "x".repeat(2001)]) {
    const { value, calls } = await withBackend(guest, () =>
      submitJoin("guest-session", message)
    )
    assert.equal(value.status, 400)
    assert.equal(calls.length, 0)
  }
  for (const code of [3106, 2109, 4000]) {
    const { value } = await withBackend(
      guest,
      () => submitJoin("guest-session", "Please consider me"),
      code
    )
    assert.equal(value.status, code === 3106 ? 403 : 400)
    assert.ok(value.data?.message)
    assert.equal(value.data?.submitted, undefined)
  }
})
