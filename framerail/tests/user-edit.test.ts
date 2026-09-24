import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule("/src/routes/[x+2d]/user/+page.server.ts")

type Rpc = { method: string; params: unknown; id: string | number }

const savedUser = {
  user_id: 42,
  name: "Alice",
  slug: "alice",
  email: "alice@example.com",
  password: "$argon2id$v=19$m=19456,t=2,p=1$secret-hash",
  multi_factor_secret: "JBSWY3DPEHPK3PXP",
  multi_factor_recovery_codes: ["recovery-1"],
  real_name: "Alice Liddell",
  locales: ["en"]
}

test("profile edit never returns the password hash or MFA secret to the browser", async () => {
  const previousFetch = globalThis.fetch
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as Rpc
    const result =
      rpc.method === "session_get"
        ? {
            user_id: 42,
            session_token: "session-secret",
            expires_at: "2099-01-01T00:00:00Z"
          }
        : rpc.method === "user_edit"
          ? savedUser
          : null
    if (result === null) throw new Error(`unexpected RPC ${rpc.method}`)
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, result }))
  }
  try {
    const response = await actions.userEdit({
      request: new Request("http://local.test/-/user", {
        method: "POST",
        body: new URLSearchParams({ realName: "Alice Liddell" })
      }),
      cookies: { get: () => "session-secret" },
      getClientAddress: () => "127.0.0.1"
    })
    const returned = JSON.stringify(response)
    assert.ok(returned.includes("Alice Liddell"), returned)
    for (const secret of ["secret-hash", "JBSWY3DPEHPK3PXP", "recovery-1"]) {
      assert.ok(!returned.includes(secret), `leaked ${secret}: ${returned}`)
    }
  } finally {
    globalThis.fetch = previousFetch
  }
})
