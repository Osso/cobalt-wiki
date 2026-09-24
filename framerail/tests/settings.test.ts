import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

import type { loadSettingsPage } from "../src/lib/server/load/settings.ts"

const { vite, close } = await createSsrServer()
after(close)
const { load, actions } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/settings/+page.server.ts"
)
const { default: SettingsPage } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/settings/+page.svelte"
)
// The component's Svelte runtime, not a second copy loaded by Node.
const { render } = await vite.ssrLoadModule("svelte/server")

const SITE_HEADERS = { "X-Wikijump-Site-Id": "6000011", "X-Wikijump-Site-Slug": "cobalt" }

const alice = {
  user_id: 42,
  name: "Alice",
  slug: "alice",
  email: "alice@example.com",
  real_name: "Alice Liddell",
  gender: null,
  birthday: "1990-05-04",
  location: "Oxford",
  biography: "Curious.",
  website: "https://alice.example",
  user_page: null,
  locales: ["en"]
}

type Rpc = { method: string; params: unknown; id: string | number }
type SettingsData = Awaited<ReturnType<typeof loadSettingsPage>>
// A saved result, or a SvelteKit ActionFailure wrapping the failed one.
type SaveResult = {
  section?: string
  saved?: boolean
  message?: string
  status?: number
  data?: { section: string; message: string }
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
    const response = responder(rpc)
    return new Response(
      JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...(response as object) })
    )
  }
  try {
    return { value: await run(), calls }
  } finally {
    globalThis.fetch = previousFetch
  }
}

function chromeResponder(rpc: Rpc) {
  if (rpc.method === "page_view") {
    return {
      result: { type: "missing", data: { compiled_top_bar_html: "<nav>Top</nav>" } }
    }
  }
  if (rpc.method === "translate") {
    return { result: { "footer-license-unless": "License" } }
  }
  throw new Error(`unexpected RPC ${rpc.method}`)
}

function loadSettings(user: typeof alice | null) {
  return withDeepwell(chromeResponder, () =>
    (load as (event: unknown) => Promise<SettingsData>)({
      request: new Request("http://local.test/-/settings", { headers: SITE_HEADERS }),
      cookies: { get: () => (user ? "session-secret" : undefined) },
      parent: async () => ({
        locales: ["en"],
        license_name: "CC BY-SA 3.0",
        license_url: "https://creativecommons.org/licenses/by-sa/3.0/",
        user_session: user ? { session: {}, user } : null
      })
    })
  )
}

/**
 * Posts the form to an action with the given session cookie and deepwell
 * replies.
 */
function submit(
  action: string,
  fields: Record<string, string>,
  responder: (rpc: Rpc) => unknown,
  sessionToken: string | null = "session-secret"
) {
  return withDeepwell(responder, () =>
    (actions[action] as (event: unknown) => Promise<SaveResult>)({
      request: new Request("http://local.test/-/settings", {
        method: "POST",
        headers: { ...SITE_HEADERS, "User-Agent": "test-agent" },
        body: new URLSearchParams(fields)
      }),
      cookies: { get: () => sessionToken ?? undefined },
      getClientAddress: () => "203.0.113.9"
    })
  )
}

function accountResponder(extra: (rpc: Rpc) => unknown = () => undefined) {
  return (rpc: Rpc) => {
    const reply = extra(rpc)
    if (reply) return reply
    if (rpc.method === "user_view") {
      return { result: { type: "user_found", data: { user: alice } } }
    }
    if (rpc.method === "user_edit") return { result: alice }
    throw new Error(`unexpected RPC ${rpc.method}`)
  }
}

test("logged-out visitors get a sign-in link back to settings and no forms", async () => {
  const { value: data } = await loadSettings(null)
  assert.equal(data.account, null)
  assert.equal(data.compiled_top_bar_html, "<nav>Top</nav>")

  const { body } = render(SettingsPage, { props: { data, form: null } })
  assert.match(body, /href="\/-\/login\?origUrl=%2F-%2Fsettings"/)
  assert.doesNotMatch(body, /<form/)
})

test("signed-in loader returns the session user's account and profile values", async () => {
  const { value: data } = await loadSettings(alice)
  assert.deepEqual(data.account, {
    name: "Alice",
    slug: "alice",
    email: "alice@example.com"
  })
  assert.equal(data.profile?.realName, "Alice Liddell")
  assert.equal(data.profile?.birthday, "1990-05-04")

  const { body } = render(SettingsPage, { props: { data, form: null } })
  assert.match(body, /alice@example\.com/)
  assert.match(body, /href="\/-\/user\/alice"/)
  assert.match(body, /name="realName"[^>]*value="Alice Liddell"/)
  assert.match(body, /action="\?\/password"/)
})

test("profile save edits only the session user's profile fields", async () => {
  const { value, calls } = await submit(
    "profile",
    {
      realName: "Alice L.",
      location: "",
      biography: "Down the hole.",
      user: "7",
      userId: "7",
      name: "Mallory",
      email: "mallory@example.com",
      password: "hijack"
    },
    accountResponder()
  )

  assert.deepEqual(value, {
    section: "profile",
    saved: true,
    message: "Your settings were saved."
  })
  assert.deepEqual(
    calls.map((call) => call.method),
    ["user_view", "user_edit"]
  )
  assert.equal(
    (calls[0].params as { session_token: string }).session_token,
    "session-secret"
  )
  assert.deepEqual(calls[1].params, {
    user: 42,
    ip_address: "203.0.113.9",
    real_name: "Alice L.",
    location: null,
    biography: "Down the hole."
  })
})

test("profile save without a session is refused before any backend call", async () => {
  const { value, calls } = await submit(
    "profile",
    { realName: "Nobody" },
    accountResponder(),
    null
  )
  assert.equal(value.status, 401)
  assert.equal(value.data?.message, "You are not signed in.")
  assert.equal(calls.length, 0)
})

test("password change checks the current password, then sets the new one", async () => {
  const { value, calls } = await submit(
    "password",
    {
      currentPassword: "old-secret",
      newPassword: "new-secret",
      confirmPassword: "new-secret"
    },
    accountResponder((rpc) => {
      if (rpc.method === "login") {
        return { result: { session_token: "check-token", needs_mfa: false } }
      }
      if (rpc.method === "logout") return { result: null }
    })
  )

  assert.equal(value.saved, true)
  assert.deepEqual(
    calls.map((call) => call.method),
    ["user_view", "login", "logout", "user_edit"]
  )
  assert.deepEqual(calls[1].params, {
    name_or_email: "alice",
    password: "old-secret",
    ip_address: "203.0.113.9",
    user_agent: "test-agent"
  })
  assert.deepEqual(calls[2].params, ["check-token"])
  assert.deepEqual(calls[3].params, {
    user: 42,
    ip_address: "203.0.113.9",
    password: "new-secret"
  })
})

test("wrong current password saves nothing and says so", async () => {
  const { value, calls } = await submit(
    "email",
    { currentPassword: "wrong", email: "alice@new.example" },
    accountResponder((rpc) => {
      if (rpc.method === "login") {
        return { error: { code: 3000, message: "Invalid authentication" } }
      }
    })
  )

  assert.equal(value.status, 400)
  assert.equal(value.data?.message, "Your current password is incorrect.")
  assert.ok(!calls.some((call) => call.method === "user_edit"))
})

test("mismatched new passwords are rejected before any backend call", async () => {
  const { value, calls } = await submit(
    "password",
    { currentPassword: "old-secret", newPassword: "one", confirmPassword: "two" },
    accountResponder()
  )
  assert.equal(value.status, 400)
  assert.equal(value.data?.message, "The new passwords do not match.")
  assert.equal(calls.length, 0)
})

test("backend errors on save are shown in the failed section", async () => {
  const { value } = await submit(
    "profile",
    { website: "not a url" },
    accountResponder((rpc) => {
      if (rpc.method === "user_edit") {
        return { error: { code: 4000, message: "Website failed the filter" } }
      }
    })
  )
  assert.equal(value.status, 500)
  assert.deepEqual(value.data, {
    section: "profile",
    message: "Website failed the filter"
  })

  const { value: data } = await loadSettings(alice)
  const { body } = render(SettingsPage, { props: { data, form: value.data } })
  assert.match(
    body,
    /class="settings-result error[^"]*"[^>]*>\s*Website failed the filter/
  )
})
