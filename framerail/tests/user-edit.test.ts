import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule("/src/routes/[x+2d]/user/+page.server.ts")

type Rpc = {
  method: string
  params: { real_name?: string }
  id: string | number
}

type Session = {
  user_id: number
  session_token: string
  created_at: string
  expires_at: string
  ip_address: string
  user_agent: string
  restricted: boolean
}

const unrestrictedSession: Session = {
  user_id: 42,
  session_token: "session-secret",
  created_at: "2026-10-03T00:00:00Z",
  expires_at: "2099-01-01T00:00:00Z",
  ip_address: "127.0.0.1",
  user_agent: "profile-test",
  restricted: false
}

const savedUser = {
  user_id: 42,
  name: "Alice",
  slug: "alice",
  email: "alice@example.com",
  password: "$argon2id$v=19$m=19456,t=2,p=1$secret-hash",
  multi_factor_secret: "JBSWY3DPEHPK3PXP",
  multi_factor_recovery_codes: ["recovery-1"],
  real_name: "Alice Original",
  locales: ["en"]
}

async function editProfile(session: Session | null, token = "session-secret") {
  const user = structuredClone(savedUser)
  const previousFetch = globalThis.fetch
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as Rpc
    let result
    if (rpc.method === "session_get") {
      result = session
    } else if (rpc.method === "user_edit") {
      if (rpc.params.real_name !== undefined) user.real_name = rpc.params.real_name
      result = user
    } else {
      throw new Error(`unexpected RPC ${rpc.method}`)
    }
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, result }))
  }
  try {
    const response = await actions.userEdit({
      request: new Request("http://local.test/-/user", {
        method: "POST",
        body: new URLSearchParams({ realName: "Alice Liddell" })
      }),
      cookies: { get: () => token || undefined },
      getClientAddress: () => "127.0.0.1"
    })
    return { response, user }
  } finally {
    globalThis.fetch = previousFetch
  }
}

test("unrestricted profile edit saves changes without returning password or MFA secrets", async () => {
  const { response, user } = await editProfile(unrestrictedSession)
  assert.equal(response.res.real_name, "Alice Liddell")
  assert.deepEqual(user, { ...savedUser, real_name: "Alice Liddell" })
  const returned = JSON.stringify(response)
  for (const secret of ["secret-hash", "JBSWY3DPEHPK3PXP", "recovery-1"]) {
    assert.ok(!returned.includes(secret), `leaked ${secret}: ${returned}`)
  }
})

test("restricted profile edit returns 403 without changing the saved user", async () => {
  const { response, user } = await editProfile({
    ...unrestrictedSession,
    restricted: true
  })
  assert.deepEqual(user, savedUser)
  assert.equal(response.status, 403)
})

for (const { name, token } of [
  { name: "missing cookie", token: "" },
  { name: "null session", token: "expired-session" }
]) {
  test(`profile edit with ${name} returns 401 without changing the saved user`, async () => {
    const { response, user } = await editProfile(null, token)
    assert.deepEqual(user, savedUser)
    assert.equal(response.status, 401)
  })
}
