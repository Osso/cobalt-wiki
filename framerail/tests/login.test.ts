import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule("/src/routes/[x+2d]/login/+page.server.ts")

type LoginReply =
  | { result: { session_token: string; needs_mfa: boolean } }
  | { error: { code: number; message: string } }
type CookieWrite = {
  name: string
  value: string | null
  options: Record<string, unknown>
}

async function signIn(
  reply: LoginReply,
  rememberMe: boolean,
  password = "correct password"
) {
  const writes: CookieWrite[] = []
  const previousFetch = globalThis.fetch
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body))
    assert.equal(rpc.method, "login")
    assert.deepEqual(rpc.params, {
      name_or_email: "alice@example.com",
      password,
      ip_address: "203.0.113.9",
      user_agent: "login-action-test"
    })
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...reply }))
  }
  try {
    const response = await actions.default({
      request: new Request("https://local.test/-/login", {
        method: "POST",
        headers: { "User-Agent": "login-action-test" },
        body: new URLSearchParams({
          nameOrEmail: "alice@example.com",
          password,
          rememberMe: rememberMe ? "on" : ""
        })
      }),
      cookies: {
        set(name: string, value: string, options: Record<string, unknown>) {
          writes.push({ name, value, options })
        },
        delete(name: string, options: Record<string, unknown>) {
          writes.push({ name, value: null, options })
        }
      },
      getClientAddress: () => "203.0.113.9"
    })
    return { response, writes }
  } finally {
    globalThis.fetch = previousFetch
  }
}

for (const rememberMe of [true, false]) {
  test(`accepted login preserves cookies and success with rememberMe=${rememberMe}`, async () => {
    const { response, writes } = await signIn(
      { result: { session_token: "accepted-session", needs_mfa: false } },
      rememberMe
    )
    assert.equal(response.isLoggedIn, true)
    assert.equal(response.session_token, "accepted-session")
    assert.equal(response.form.valid, true)
    assert.deepEqual(response.form.data, {
      nameOrEmail: "alice@example.com",
      password: "correct password",
      rememberMe
    })
    const options = {
      path: "/",
      httpOnly: true,
      secure: true,
      sameSite: "lax",
      ...(rememberMe ? { maxAge: 60 * 60 * 24 * 400 } : {})
    }
    assert.deepEqual(writes, [
      { name: "wikijump_token", value: "accepted-session", options },
      { name: "wikijump_remember", value: rememberMe ? "1" : null, options }
    ])
  })

  test(`pending MFA denies login without cookies or token with rememberMe=${rememberMe}`, async () => {
    const { response, writes } = await signIn(
      { result: { session_token: "pending-session-secret", needs_mfa: true } },
      rememberMe
    )
    assert.equal(response.status, 403)
    assert.deepEqual(writes, [])
    assert.equal(response.data.form.valid, true)
    assert.deepEqual(response.data.form.data, {
      nameOrEmail: "alice@example.com",
      password: "correct password",
      rememberMe
    })
    assert.equal(
      response.data.message,
      "Multi-factor authentication (MFA) is required. This sign-in page cannot complete it."
    )
    assert.equal(response.isLoggedIn, undefined)
    assert.equal(response.data.isLoggedIn, undefined)
    assert.equal(response.session_token, undefined)
    assert.equal(response.data.session_token, undefined)
    assert.ok(!JSON.stringify(response).includes("pending-session-secret"))
  })
}

test("wrong credentials fail as unauthorized and write no cookies", async () => {
  const { response, writes } = await signIn(
    { error: { code: 3000, message: "Invalid username, password, or TOTP code" } },
    true,
    "wrong password"
  )
  assert.equal(response.status, 401)
  assert.equal(response.data.code, 3000)
  assert.equal(response.data.message, "Invalid username, password, or TOTP code")
  assert.equal(response.data.form.valid, true)
  assert.deepEqual(response.data.form.data, {
    nameOrEmail: "alice@example.com",
    password: "wrong password",
    rememberMe: true
  })
  assert.equal(response.isLoggedIn, undefined)
  assert.equal(response.data.isLoggedIn, undefined)
  assert.equal(response.data.session_token, undefined)
  assert.deepEqual(writes, [])
})
