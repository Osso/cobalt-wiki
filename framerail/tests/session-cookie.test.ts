import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { setSessionCookie, refreshRememberedSession, clearSessionCookies } =
  await vite.ssrLoadModule("/src/lib/server/auth/sessionCookie.ts")

const FOUR_HUNDRED_DAYS = 60 * 60 * 24 * 400
const TOKEN = "wj:0123456789abcdef0123456789abcdef0123456789abcdef0123"

/**
 * A browser cookie jar: set/delete record Set-Cookie calls and update
 * values.
 */
function jar(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial))
  const calls: {
    name: string
    value: string | null
    options: Record<string, unknown>
  }[] = []
  return {
    calls,
    values,
    get: (name: string) => values.get(name),
    set(name: string, value: string, options: Record<string, unknown>) {
      values.set(name, value)
      calls.push({ name, value, options })
    },
    delete(name: string, options: Record<string, unknown>) {
      values.delete(name)
      calls.push({ name, value: null, options })
    }
  }
}

test("remembered sign-in stores the token for 400 days with a remember flag", () => {
  const cookies = jar()
  setSessionCookie(cookies, TOKEN, true)
  assert.equal(cookies.values.get("wikijump_token"), TOKEN)
  assert.equal(cookies.values.get("wikijump_remember"), "1")
  for (const call of cookies.calls) {
    assert.equal(call.options.maxAge, FOUR_HUNDRED_DAYS)
    assert.equal(call.options.httpOnly, true)
    assert.equal(call.options.secure, true)
    assert.equal(call.options.path, "/")
  }
})

test("plain sign-in stores a browser-session token and drops any remember flag", () => {
  const cookies = jar({ wikijump_remember: "1" })
  setSessionCookie(cookies, TOKEN, false)
  const token = cookies.calls.find((call) => call.name === "wikijump_token")
  assert.equal(token?.value, TOKEN)
  assert.equal(token?.options.maxAge, undefined)
  assert.equal(token?.options.expires, undefined)
  assert.equal(cookies.values.has("wikijump_remember"), false)
})

test("visits push a remembered session out again; plain sessions are left alone", () => {
  const remembered = jar({ wikijump_token: TOKEN, wikijump_remember: "1" })
  refreshRememberedSession(remembered)
  assert.deepEqual(
    remembered.calls.map((call) => [call.name, call.value, call.options.maxAge]),
    [
      ["wikijump_token", TOKEN, FOUR_HUNDRED_DAYS],
      ["wikijump_remember", "1", FOUR_HUNDRED_DAYS]
    ]
  )

  const plain = jar({ wikijump_token: TOKEN })
  refreshRememberedSession(plain)
  assert.deepEqual(plain.calls, [])
})

test("sign-out clears both cookies", () => {
  const cookies = jar({ wikijump_token: TOKEN, wikijump_remember: "1" })
  clearSessionCookies(cookies)
  assert.equal(cookies.values.size, 0)
  assert.deepEqual(
    cookies.calls.map((call) => call.name),
    ["wikijump_token", "wikijump_remember"]
  )
})
