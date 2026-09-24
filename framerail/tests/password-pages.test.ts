import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const setPassword = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/set-password/[token]/+page.server.ts"
)
const forgotPassword = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/forgot-password/+page.server.ts"
)
const { default: SetPasswordPage } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/set-password/[token]/+page.svelte"
)
const { default: ForgotPasswordPage } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/forgot-password/+page.svelte"
)
const { default: LoginPage } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/login/+page.svelte"
)
const { render } = await vite.ssrLoadModule("svelte/server")
const { readable } = await vite.ssrLoadModule("svelte/store")

const SITE_HEADERS = { "X-Wikijump-Site-Id": "6000011", "X-Wikijump-Site-Slug": "cobalt" }

type Rpc = { method: string; params: unknown; id: string | number }
// A result, or a SvelteKit ActionFailure wrapping the failed one.
type ActionResult = {
  saved?: boolean
  sent?: boolean
  status?: number
  data?: { message: string; linkUnusable?: boolean }
}

async function withDeepwell<T>(
  responder: (rpc: Rpc) => unknown,
  run: () => Promise<T>
): Promise<{ value: T; calls: Rpc[] }> {
  const previousFetch = globalThis.fetch
  const calls: Rpc[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as Rpc
    calls.push(rpc)
    return new Response(
      JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...(responder(rpc) as object) })
    )
  }
  try {
    return { value: await run(), calls }
  } finally {
    globalThis.fetch = previousFetch
  }
}

function submit(
  module: Record<string, unknown>,
  url: string,
  fields: Record<string, string>,
  responder: (rpc: Rpc) => unknown,
  params: Record<string, string> = {}
) {
  return withDeepwell(responder, () =>
    (module.actions as { default: (event: unknown) => Promise<ActionResult> }).default({
      request: new Request(`http://local.test${url}`, {
        method: "POST",
        headers: SITE_HEADERS,
        body: new URLSearchParams(fields)
      }),
      params,
      getClientAddress: () => "203.0.113.9"
    })
  )
}

function setPasswordSubmit(
  fields: Record<string, string>,
  responder: (rpc: Rpc) => unknown
) {
  return submit(setPassword, "/-/set-password/tok-abc", fields, responder, {
    token: "tok-abc"
  })
}

const unexpected = (rpc: Rpc) => {
  throw new Error(`unexpected RPC ${rpc.method}`)
}

test("set-password sends the link's token and new password, then offers sign-in", async () => {
  const { value, calls } = await setPasswordSubmit(
    { newPassword: "chosen secret", confirmPassword: "chosen secret" },
    () => ({ result: null })
  )
  assert.deepEqual(value, { saved: true })
  assert.deepEqual(calls, [
    {
      jsonrpc: "2.0",
      id: calls[0].id,
      method: "password_token_redeem",
      params: { token: "tok-abc", password: "chosen secret", ip_address: "203.0.113.9" }
    }
  ])

  const { body } = render(SetPasswordPage, { props: { data: {}, form: value } })
  assert.match(body, /Your password is set\./)
  assert.match(body, /href="\/-\/login"/)
  assert.doesNotMatch(body, /<form/)
})

test("set-password refuses empty or mismatched passwords before any backend call", async () => {
  for (const [fields, message] of [
    [{ newPassword: "", confirmPassword: "" }, "Enter a new password."],
    [{ newPassword: "one", confirmPassword: "two" }, "The passwords do not match."]
  ] as const) {
    const { value, calls } = await setPasswordSubmit(fields, unexpected)
    assert.equal(value.status, 400)
    assert.equal(value.data?.message, message)
    assert.equal(calls.length, 0)
  }
})

test("unknown, expired and used links each get their own message and a new-link offer", async () => {
  const expected = {
    3007: "This link is not valid. Ask for a new one below.",
    3008: "This link has expired. Ask for a new one below.",
    3009: "This link has already been used. Sign in with your password, or ask for a new link below."
  }
  for (const [code, message] of Object.entries(expected)) {
    const { value } = await setPasswordSubmit(
      { newPassword: "chosen secret", confirmPassword: "chosen secret" },
      () => ({ error: { code: Number(code), message: "Deepwell summary" } })
    )
    assert.equal(value.status, 400)
    assert.deepEqual(value.data, { message, linkUnusable: true })

    const { body } = render(SetPasswordPage, { props: { data: {}, form: value.data } })
    assert.match(body, /class="password-result error[^"]*"[^>]*>[^<]*This link/)
    assert.match(body, /href="\/-\/forgot-password"/)
    assert.match(body, /name="newPassword"/)
  }
})

test("forgot-password asks Deepwell for the site and shows a neutral confirmation", async () => {
  const { value, calls } = await submit(
    forgotPassword,
    "/-/forgot-password",
    { email: "  alice@example.com " },
    () => ({ result: null })
  )
  assert.deepEqual(value, { sent: true })
  assert.deepEqual(
    calls.map((call) => [call.method, call.params]),
    [["password_reset_request", { email: "alice@example.com", site_id: 6000011 }]]
  )

  const { body } = render(ForgotPasswordPage, { props: { data: {}, form: value } })
  assert.match(body, /If an account uses that address, we have emailed it a link/)
  assert.doesNotMatch(body, /<form/)
})

test("forgot-password needs an address and hides backend failure details", async () => {
  const { value: empty, calls } = await submit(
    forgotPassword,
    "/-/forgot-password",
    { email: " " },
    unexpected
  )
  assert.equal(empty.status, 400)
  assert.equal(empty.data?.message, "Enter your email address.")
  assert.equal(calls.length, 0)

  const { value: failed } = await submit(
    forgotPassword,
    "/-/forgot-password",
    { email: "alice@example.com" },
    () => ({ error: { code: 0, message: "email sending is not configured" } })
  )
  assert.equal(failed.status, 500)
  assert.deepEqual(failed.data, {
    message: "The email could not be sent. Try again later."
  })
})

test("the sign-in page links to the forgotten-password page", () => {
  // SvelteKit's request context, which the page's form and page state read.
  const page = {
    url: new URL("http://local.test/-/login"),
    data: { site: { name: "Cobalt Company" } },
    form: null,
    params: {},
    route: { id: "/[x+2d]/login" },
    state: {},
    status: 200,
    error: null
  }
  const context = new Map<string, unknown>([
    ["__request__", { page }],
    [
      "__svelte__",
      {
        page: readable(page),
        navigating: readable(null),
        updated: { subscribe: readable(false).subscribe, check: async () => false }
      }
    ]
  ])
  const { body } = render(LoginPage, {
    context,
    props: {
      data: {
        isLoggedIn: false,
        internationalization: {},
        loginForm: {
          id: "login",
          valid: false,
          posted: false,
          errors: {},
          data: { nameOrEmail: "", password: "" }
        }
      }
    }
  })
  assert.match(body, /<a href="\/-\/forgot-password"[^>]*>Forgotten your password\?<\/a>/)
})
