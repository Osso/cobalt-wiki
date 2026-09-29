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
  locales: ["en"],
  avatar_s3_hash: null as number[] | null
}

type Rpc = { method: string; params: unknown; id: string | number }
type SettingsData = Awaited<ReturnType<typeof loadSettingsPage>>
// A saved result, or a SvelteKit ActionFailure wrapping the failed one.
type SaveResult = {
  section?: string
  saved?: boolean
  watching?: boolean
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
  if (rpc.method === "watching_preferences_get") {
    return { result: { email_enabled: false, auto_watch: false } }
  }
  if (rpc.method === "watching_subscriptions") {
    return { result: [{ scope: "page", target_id: 12 }] }
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
    userId: 42,
    name: "Alice",
    slug: "alice",
    email: "alice@example.com",
    avatarHash: null
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

test("settings show opt-in preferences and let users remove subscriptions without page visibility", async () => {
  const loaded = await loadSettings(alice)
  assert.deepEqual(loaded.value.watchingPreferences, {
    email_enabled: false,
    auto_watch: false
  })
  assert.deepEqual(loaded.value.watchingSubscriptions, [{ scope: "page", target_id: 12 }])
  const { body } = render(SettingsPage, { props: { data: loaded.value, form: null } })
  assert.match(body, /name="email_enabled"/)
  assert.match(body, /name="auto_watch"/)
  assert.match(body, /action="\?\/watching"/)
  assert.match(body, /action="\?\/unwatch"/)
  assert.match(body, /name="target_id"[^>]*value="12"/)
})

test("a user can unwatch an inaccessible page through their subscription list", async () => {
  const { value, calls } = await submit(
    "unwatch",
    {
      scope: "page",
      target_id: "12",
      watching: "false"
    },
    (rpc) =>
      rpc.method === "watching_subscription_set"
        ? { result: { watching: false } }
        : chromeResponder(rpc)
  )
  assert.equal(value.watching, false)
  assert.deepEqual(
    calls.map((call) => [call.method, call.params]),
    [
      [
        "watching_subscription_set",
        { site_id: 6000011, scope: "page", target_id: 12, watching: false }
      ]
    ]
  )
})

test("watcher email opt-out preserves subscriptions and requires explicit Save", async () => {
  const { value, calls } = await submit(
    "watching",
    { email_enabled: "", auto_watch: "on", user_id: "9" },
    (rpc) =>
      rpc.method === "watching_preferences_set"
        ? { result: { email_enabled: false, auto_watch: true } }
        : chromeResponder(rpc)
  )
  assert.equal(value.saved, true)
  assert.deepEqual(
    calls.map((call) => [call.method, call.params]),
    [["watching_preferences_set", { email_enabled: false, auto_watch: true }]]
  )
})

test("unverified email rejection appears beside watcher preferences", async () => {
  const { value } = await submit("watching", { email_enabled: "on" }, (rpc) => {
    if (rpc.method === "watching_preferences_set") {
      return {
        error: { code: 4000, message: "Verify your email before enabling watcher email." }
      }
    }
    return chromeResponder(rpc)
  })
  assert.equal(value.status, 400)
  const { value: data } = await loadSettings(alice)
  const { body } = render(SettingsPage, { props: { data, form: value.data } })
  assert.match(body, /Verify your email before enabling watcher email/)
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

const PNG_BYTES = new Uint8Array([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3
])

type AvatarCalls = { rpcs: Rpc[]; puts: { url: string; bytes: number[] }[] }

/**
 * Posts a multipart avatar form. RPCs go to `responder`; the presigned
 * upload PUT is recorded and answered with `putStatus`.
 */
async function submitAvatar(
  fields: Record<string, string | File>,
  responder: (rpc: Rpc) => unknown,
  putStatus = 200
): Promise<{ value: SaveResult; calls: AvatarCalls }> {
  const previousFetch = globalThis.fetch
  const calls: AvatarCalls = { rpcs: [], puts: [] }
  globalThis.fetch = async (input, init) => {
    if (init?.method === "PUT") {
      const body = init.body as Blob
      calls.puts.push({
        url: String(input),
        bytes: [...new Uint8Array(await body.arrayBuffer())]
      })
      return new Response(null, { status: putStatus })
    }
    const rpc = JSON.parse(String(init?.body)) as Rpc
    calls.rpcs.push(rpc)
    return new Response(
      JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...(responder(rpc) as object) })
    )
  }
  const body = new FormData()
  for (const [key, value] of Object.entries(fields)) body.set(key, value)
  try {
    const value = await (actions.avatar as (event: unknown) => Promise<SaveResult>)({
      request: new Request("http://local.test/-/settings", {
        method: "POST",
        headers: { ...SITE_HEADERS, "User-Agent": "test-agent" },
        body
      }),
      cookies: { get: () => "session-secret" },
      getClientAddress: () => "203.0.113.9"
    })
    return { value, calls }
  } finally {
    globalThis.fetch = previousFetch
  }
}

const presignResponder = accountResponder((rpc) =>
  rpc.method === "blob_upload"
    ? {
        result: {
          pending_blob_id: "pending-7",
          presign_url: "https://files.example/upload?sig=abc"
        }
      }
    : undefined
)

test("avatar upload stores the image as the session user's avatar", async () => {
  const file = new File([PNG_BYTES], "me.webp", { type: "image/webp" })
  const { value, calls } = await submitAvatar(
    { avatar: file, user: "7" },
    presignResponder
  )

  assert.deepEqual(value, {
    section: "avatar",
    saved: true,
    message: "Your settings were saved."
  })
  assert.deepEqual(
    calls.rpcs.map((call) => call.method),
    ["user_view", "blob_upload", "user_edit"]
  )
  assert.deepEqual(calls.rpcs[1].params, { user_id: 42, blob_size: PNG_BYTES.length })
  assert.deepEqual(calls.puts, [
    { url: "https://files.example/upload?sig=abc", bytes: [...PNG_BYTES] }
  ])
  assert.deepEqual(calls.rpcs[2].params, {
    user: 42,
    ip_address: "203.0.113.9",
    avatar_uploaded_blob_id: "pending-7"
  })
})

test("avatar removal clears the stored avatar", async () => {
  const { value, calls } = await submitAvatar({ remove: "1" }, accountResponder())

  assert.equal(value.saved, true)
  assert.deepEqual(calls.rpcs[1].params, {
    user: 42,
    ip_address: "203.0.113.9",
    avatar_uploaded_blob_id: null
  })
})

test("avatar upload rejects non-images before any backend call", async () => {
  const file = new File(["hello"], "notes.txt", { type: "text/plain" })
  const { value, calls } = await submitAvatar({ avatar: file }, presignResponder)

  assert.equal(value.status, 400)
  assert.deepEqual(value.data, { section: "avatar", message: "Choose an image file." })
  assert.deepEqual(calls.rpcs, [])
  assert.deepEqual(calls.puts, [])
})

test("a failed avatar upload does not change the avatar", async () => {
  const file = new File([PNG_BYTES], "me.webp", { type: "image/webp" })
  const { value, calls } = await submitAvatar({ avatar: file }, presignResponder, 403)

  assert.equal(value.status, 500)
  assert.equal(value.data?.section, "avatar")
  assert.match(value.data?.message ?? "", /upload failed \(403\)/i)
  assert.deepEqual(
    calls.rpcs.map((call) => call.method),
    ["user_view", "blob_upload"]
  )
})

test("settings page shows the current avatar with upload and remove controls", async () => {
  const { value: data } = await loadSettings({ ...alice, avatar_s3_hash: [171, 205] })
  assert.equal(data.account?.userId, 42)
  assert.deepEqual(data.account?.avatarHash, [171, 205])

  const { body } = render(SettingsPage, { props: { data, form: null } })
  assert.match(body, /action="\?\/avatar"[^>]*enctype="multipart\/form-data"/)
  assert.match(body, /src="\/-\/avatar\/42\?v=abcd"/)
  assert.match(body, /name="avatar"[^>]*accept="image\/\*"/)
  assert.match(body, /name="remove"/)
})

/** The ids of the category sections, split into shown and hidden. */
function categorySections(body: string) {
  const sections = [...body.matchAll(/<section id="(\w+)"[^>]*>/g)]
  return {
    shown: sections.filter(([tag]) => !/\shidden/.test(tag)).map(([, id]) => id),
    hidden: sections.filter(([tag]) => /\shidden/.test(tag)).map(([, id]) => id)
  }
}

test("settings open on Profile with a sidebar of every category", async () => {
  const { value: data } = await loadSettings(alice)
  const { body } = render(SettingsPage, { props: { data, form: null } })

  const nav = body.slice(body.indexOf('<nav class="settings-nav'), body.indexOf("</nav>"))
  assert.deepEqual(
    [...nav.matchAll(/href="#(\w+)"[^>]*>([^<]+)</g)].map(
      ([, id, label]) => `${id}:${label}`
    ),
    ["profile:Profile", "email:Email", "password:Password", "watching:Watching"]
  )
  assert.match(
    nav,
    /aria-current="page"[^>]*href="#profile"|href="#profile"[^>]*aria-current="page"/
  )
  assert.deepEqual(categorySections(body), {
    shown: ["profile"],
    hidden: ["email", "password", "watching"]
  })
})

test("a save result opens the category of the submitted form", async () => {
  const { value: data } = await loadSettings(alice)
  const emailSaved = {
    section: "email",
    saved: true,
    message: "Your settings were saved."
  }
  const email = render(SettingsPage, { props: { data, form: emailSaved } }).body
  assert.deepEqual(categorySections(email).shown, ["email"])
  assert.match(email, /<section id="email"[^>]*>[\s\S]*Your settings were saved\./)

  const avatarSaved = {
    section: "avatar",
    saved: true,
    message: "Your settings were saved."
  }
  const avatar = render(SettingsPage, { props: { data, form: avatarSaved } }).body
  assert.deepEqual(categorySections(avatar).shown, ["profile"])
})
