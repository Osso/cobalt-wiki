import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { load, actions } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/login/+page.server.ts"
)

type LoginReply =
  | { result: { session_token: string; needs_mfa: boolean } }
  | { error: { code: number; message: string } }

async function withRpc(invoke: () => Promise<unknown>, login?: LoginReply) {
  const previous = globalThis.fetch
  globalThis.fetch = async (_input, init) => {
    const request = JSON.parse(String(init?.body))
    const reply =
      request.method === "login"
        ? login
        : {
            result:
              request.method === "page_view"
                ? { type: "found", data: { compiled_top_bar_html: "<nav>Fixture</nav>" } }
                : { "login.toast": "You have been logged in." }
          }
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: request.id, ...reply }))
  }
  try {
    return await invoke()
  } finally {
    globalThis.fetch = previous
  }
}

function requestUrl(destination?: string) {
  const url = new URL("https://local.test/-/login")
  if (destination !== undefined) url.searchParams.set("origUrl", destination)
  return url
}

function readLogin(loggedIn: boolean, destination?: string) {
  return withRpc(() =>
    load({
      request: new Request(requestUrl(destination), {
        headers: { "X-Wikijump-Site-Id": "11", "X-Wikijump-Site-Slug": "fixture" }
      }),
      cookies: { get: () => (loggedIn ? "existing-fixture-session" : undefined) },
      parent: async () => ({
        locales: ["en"],
        user_session: loggedIn ? { user: { user_id: 42 } } : null,
        license_name: "Fixture license",
        license_url: "https://example.invalid/license"
      })
    })
  )
}

const accepted = { result: { session_token: "new-fixture-session", needs_mfa: false } }

async function postLogin(destination: string, reply: LoginReply = accepted) {
  const writes: { name: string; value: string | null }[] = []
  const result = withRpc(
    () =>
      actions.default({
        request: new Request(requestUrl(destination), {
          method: "POST",
          body: new URLSearchParams({
            nameOrEmail: "fixture@example.invalid",
            password: "fixture password",
            rememberMe: "on"
          })
        }),
        cookies: {
          set: (name: string, value: string) => writes.push({ name, value }),
          delete: (name: string) => writes.push({ name, value: null })
        },
        getClientAddress: () => "127.0.0.1"
      }),
    reply
  )
  return { result, writes }
}

for (const destination of ["/player:alli", "/player:alli?pane=files#portraits"]) {
  test(`authenticated login page returns to ${destination}`, async () => {
    await assert.rejects(() => readLogin(true, destination), {
      status: 303,
      location: destination
    })
  })
}

for (const destination of [
  undefined,
  "",
  "https://example.invalid/private",
  "https://local.test/player:alli",
  "//example.invalid/private",
  "/\\[",
  "/\\example.invalid/private",
  "/\n/example.invalid/private"
]) {
  test(`authenticated login does not follow unsafe or absent return ${JSON.stringify(destination)}`, async () => {
    const response = (await readLogin(true, destination)) as { isLoggedIn: boolean }
    assert.equal(response.isLoggedIn, true)
  })
}

test("anonymous return-to request retains the sign-in form", async () => {
  const response = (await readLogin(false, "/player:alli")) as {
    isLoggedIn: boolean
    loginForm: { data: { rememberMe: boolean } }
  }
  assert.equal(response.isLoggedIn, false)
  assert.equal(response.loginForm.data.rememberMe, true)
})

test("successful explicit return redirects after writing the login cookie", async () => {
  const { result, writes } = await postLogin("/player:alli")
  await assert.rejects(result, { status: 303, location: "/player:alli" })
  assert.ok(
    writes.some(
      ({ name, value }) => name === "wikijump_token" && value === "new-fixture-session"
    )
  )
})

for (const destination of [
  "https://example.invalid/private",
  "/\\example.invalid/private"
]) {
  test(`successful login does not follow unsafe return ${JSON.stringify(destination)}`, async () => {
    const { result, writes } = await postLogin(destination)
    const response = (await result) as { isLoggedIn: boolean }
    assert.equal(response.isLoggedIn, true)
    assert.ok(writes.some(({ name }) => name === "wikijump_token"))
  })
}

test("pending MFA never follows return-to or writes cookies", async () => {
  const { result, writes } = await postLogin("/player:alli", {
    result: { session_token: "pending-fixture-session", needs_mfa: true }
  })
  const response = (await result) as { status: number }
  assert.equal(response.status, 403)
  assert.deepEqual(writes, [])
})

test("invalid credentials never follow return-to or write cookies", async () => {
  const { result, writes } = await postLogin("/player:alli", {
    error: { code: 3000, message: "Invalid credentials" }
  })
  const response = (await result) as { status: number }
  assert.equal(response.status, 401)
  assert.deepEqual(writes, [])
})
