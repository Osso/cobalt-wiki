import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

import type { loadMembersPage } from "../src/lib/server/load/members.ts"

const { vite, close } = await createSsrServer()
after(close)
const { load, actions } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/admin/members/+page.server.ts"
)
const { default: MembersPage } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/admin/members/+page.svelte"
)
const { default: LoginStatus } = await vite.ssrLoadModule(
  "/src/lib/component/LoginStatus.svelte"
)
const { render } = await vite.ssrLoadModule("svelte/server")
const { readable } = await vite.ssrLoadModule("svelte/store")

const SITE_HEADERS = { "X-Wikijump-Site-Id": "6000011", "X-Wikijump-Site-Slug": "cobalt" }

const alice = { user_id: 42, name: "Alice", slug: "alice", locales: ["en"] }

const MEMBERS = [
  {
    user_id: 7,
    name: "OzmaAsimov",
    slug: "ozmaasimov",
    email: "wikidot-7@members.invalid",
    joined_at: "2020-11-03T08:00:00Z",
    role: "root"
  },
  {
    user_id: 42,
    name: "Alice",
    slug: "alice",
    email: "alice@example.com",
    joined_at: "2021-04-29T14:55:00Z",
    role: "admin"
  },
  {
    user_id: 55,
    name: "Bob",
    slug: "bob",
    email: "bob@example.com",
    joined_at: "2022-01-09T23:30:00Z",
    role: "moderator"
  },
  {
    user_id: 61,
    name: "Carol",
    slug: "carol",
    email: "carol@example.com",
    joined_at: "2023-06-15T00:00:00Z",
    role: "member"
  }
]

type Rpc = { method: string; params: unknown; id: string | number }
type Call = Rpc & { headers: Record<string, string> }
type MembersData = Awaited<ReturnType<typeof loadMembersPage>>
// A result, or a SvelteKit ActionFailure wrapping the failed one.
type ActionResult = {
  saved?: boolean
  message?: string
  status?: number
  data?: { message: string }
}

async function withDeepwell<T>(
  responder: (rpc: Rpc) => unknown,
  run: () => Promise<T>
): Promise<{ value: T; calls: Call[] }> {
  const previousFetch = globalThis.fetch
  const calls: Call[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as Rpc
    calls.push({ ...rpc, headers: init?.headers as Record<string, string> })
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

/** Site chrome replies, then `extra` for the member RPCs. */
function responder(extra: (rpc: Rpc) => unknown = () => undefined) {
  return (rpc: Rpc) => {
    const reply = extra(rpc)
    if (reply) return reply
    if (rpc.method === "page_view") {
      return {
        result: { type: "missing", data: { compiled_top_bar_html: "<nav>Top</nav>" } }
      }
    }
    if (rpc.method === "member_application_list") {
      return { result: [] }
    }
    if (rpc.method === "translate") {
      return { result: { "footer-license-unless": "License" } }
    }
    throw new Error(`unexpected RPC ${rpc.method}`)
  }
}

const listMembers = responder((rpc) =>
  rpc.method === "member_admin_list" ? { result: MEMBERS } : undefined
)

const denied = { error: { code: 3106, message: "Permission denied" } }

function loadMembers(sessionToken: string | null, reply: (rpc: Rpc) => unknown) {
  return withDeepwell(reply, () =>
    (load as (event: unknown) => Promise<MembersData>)({
      request: new Request("http://local.test/-/admin/members", {
        headers: SITE_HEADERS
      }),
      cookies: { get: () => sessionToken ?? undefined },
      parent: async () => ({
        locales: ["en"],
        license_name: "CC BY-SA 3.0",
        license_url: "https://creativecommons.org/licenses/by-sa/3.0/",
        user_session: sessionToken ? { session: {}, user: alice } : null
      })
    })
  )
}

function submit(
  action: string,
  fields: Record<string, string>,
  reply: (rpc: Rpc) => unknown,
  sessionToken: string | null = "session-secret"
) {
  return withDeepwell(reply, () =>
    (actions[action] as (event: unknown) => Promise<ActionResult>)({
      request: new Request("http://local.test/-/admin/members", {
        method: "POST",
        headers: SITE_HEADERS,
        body: new URLSearchParams(fields)
      }),
      cookies: { get: () => sessionToken ?? undefined },
      getClientAddress: () => "203.0.113.9"
    })
  )
}

function memberCalls(calls: Call[]) {
  return calls.filter((call) => call.method.startsWith("member_admin_"))
}

function renderPage(data: MembersData, form: unknown = null) {
  return render(MembersPage, { props: { data, form } }).body
}

test("admins see pending membership messages with approve and reject actions", async () => {
  const application = {
    user_id: 90,
    user_name: "New Reader",
    message: "I'd like to contribute <script>alert(1)</script>",
    created_at: "2026-09-25T06:00:00Z"
  }
  const { value: data } = await loadMembers(
    "session-secret",
    responder((rpc) => {
      if (rpc.method === "member_admin_list") return { result: MEMBERS }
      if (rpc.method === "member_application_list") return { result: [application] }
      return undefined
    })
  )
  assert.deepEqual(data.applications, [application])
  const body = renderPage(data)
  assert.match(body, /Membership applications/)
  assert.match(body, /New Reader/)
  assert.match(body, /&lt;script(?:>|&gt;)/)
  assert.doesNotMatch(body, /<script>alert/)
  assert.match(body, /Approve/)
  assert.match(body, /Reject/)
})

test("admins approve or reject applications using their session and site context", async () => {
  for (const decision of ["approve", "reject"]) {
    const { value, calls } = await submit(
      "application",
      { userId: "90", decision },
      responder((rpc) =>
        rpc.method === "member_application_decide" ? { result: {} } : undefined
      )
    )
    assert.equal(value.saved, true)
    const call = calls.find((entry) => entry.method === "member_application_decide")!
    assert.deepEqual(call.params, {
      user_id: 90,
      accept: decision === "approve",
      ip_address: "203.0.113.9"
    })
    assert.equal(call.headers["X-Deepwell-Session-Token"], "session-secret")
    assert.equal(call.headers["X-Deepwell-Site-Id"], "6000011")
  }
  const invalid = await submit(
    "application",
    { userId: "90", decision: "owner" },
    responder()
  )
  assert.equal(invalid.value.status, 400)
  assert.equal(invalid.calls.length, 0)
  const unauth = await submit(
    "application",
    { userId: "90", decision: "approve" },
    responder(),
    null
  )
  assert.equal(unauth.value.status, 401)
  assert.equal(unauth.calls.length, 0)
  const forbidden = await submit(
    "application",
    { userId: "90", decision: "approve" },
    responder(() => denied)
  )
  assert.equal(forbidden.value.status, 403)
})

test("signed-out visitors get a sign-in link and no member data is requested", async () => {
  const { value: data, calls } = await loadMembers(null, responder())
  assert.equal(data.access, "signed-out")
  assert.deepEqual(memberCalls(calls), [])

  const body = renderPage(data)
  assert.match(body, /href="\/-\/login\?origUrl=%2F-%2Fadmin%2Fmembers"/)
  assert.doesNotMatch(body, /<table/)
})

test("a signed-in non-admin gets the permission message and no list", async () => {
  const { value: data } = await loadMembers(
    "session-secret",
    responder((rpc) => (rpc.method === "member_admin_list" ? denied : undefined))
  )
  assert.equal(data.access, "denied")
  assert.deepEqual(data.members, [])

  const body = renderPage(data)
  assert.match(body, /Only administrators of this site can see and manage its members\./)
  assert.doesNotMatch(body, /<table|carol@example.com|Invite a member/)
})

test("admins see every member with email, role, join date and options", async () => {
  const { value: data, calls } = await loadMembers("session-secret", listMembers)

  const [list] = memberCalls(calls)
  assert.equal(list.method, "member_admin_list")
  assert.deepEqual(list.params, {})
  assert.equal(list.headers["X-Deepwell-Session-Token"], "session-secret")
  assert.equal(list.headers["X-Deepwell-Site-Id"], "6000011")
  assert.equal(data.access, "admin")
  assert.equal(data.viewerId, 42)

  const body = renderPage(data)
  for (const tab of ["All Members", "Moderators", "Administrators"]) {
    assert.match(body, new RegExp(`role="tab"[^>]*>${tab}</button>`))
  }
  const rows = body
    .split("<tbody")[1]
    .split(/<tr[\s>]/)
    .slice(1)
  assert.equal(rows.length, 4)
  const [root, self, moderator, member] = rows
  assert.match(root, /OzmaAsimov<\/a>/)
  assert.match(root, /\(not set\)/)
  assert.match(root, /Site owner/)
  assert.match(root, /3 Nov 2020/)
  assert.match(self, /alice@example.com/)
  assert.match(self, /Administrator/)
  assert.match(self, /You<\/span>/)
  assert.match(moderator, /bob@example.com[\s\S]*Moderator[\s\S]*9 Jan 2022/)
  assert.match(member, /carol@example.com[\s\S]*Member[\s\S]*15 Jun 2023/)
  // Only members other than the owner and the viewer can be changed.
  for (const row of [root, self]) assert.doesNotMatch(row, /\?\/role|\?\/remove/)
  for (const row of [moderator, member]) {
    assert.match(row, /action="\?\/role"/)
    assert.match(row, /action="\?\/remove"/)
  }
  assert.match(body, /action="\?\/invite"/)
})

test("role change sends the target and role; Deepwell learns the actor from the session", async () => {
  const { value, calls } = await submit(
    "role",
    { userId: "61", name: "Carol", role: "moderator" },
    responder((rpc) =>
      rpc.method === "member_admin_set_role" ? { result: null } : undefined
    )
  )
  assert.deepEqual(value, { saved: true, message: "Carol is now a moderator." })
  assert.equal(calls.length, 1)
  assert.deepEqual(calls[0].params, {
    user_id: 61,
    role: "moderator",
    ip_address: "203.0.113.9"
  })
  assert.equal(calls[0].headers["X-Deepwell-Session-Token"], "session-secret")
  assert.equal(calls[0].headers["X-Deepwell-Site-Id"], "6000011")
})

test("an unknown role is refused before any backend call", async () => {
  const { value, calls } = await submit(
    "role",
    { userId: "61", name: "Carol", role: "root" },
    responder()
  )
  assert.equal(value.status, 400)
  assert.equal(value.data?.message, "Choose Member, Moderator or Administrator.")
  assert.equal(calls.length, 0)
})

test("removal sends the member and shows the result", async () => {
  const { value, calls } = await submit(
    "remove",
    { userId: "55", name: "Bob" },
    responder((rpc) =>
      rpc.method === "member_admin_remove" ? { result: null } : undefined
    )
  )
  assert.deepEqual(value, { saved: true, message: "Bob is no longer a member." })
  assert.deepEqual(calls[0].params, { user_id: 55, ip_address: "203.0.113.9" })

  const { value: data } = await loadMembers("session-secret", listMembers)
  assert.match(
    renderPage(data, value),
    /class="members-result saved[^"]*"[^>]*>\s*Bob is no longer a member\./
  )
})

test("invite sends email and name and says whether a new account was emailed", async () => {
  const invited = (created: boolean) =>
    responder((rpc) =>
      rpc.method === "member_admin_invite"
        ? { result: { user_id: 90, created, emailed: created } }
        : undefined
    )

  const { value, calls } = await submit(
    "invite",
    { email: " dana@example.com ", name: " Dana " },
    invited(true)
  )
  assert.deepEqual(calls[0].params, {
    email: "dana@example.com",
    name: "Dana",
    ip_address: "203.0.113.9"
  })
  assert.equal(
    value.message,
    "Invited dana@example.com: the new account was emailed a link to choose a password."
  )

  const { value: existing, calls: existingCalls } = await submit(
    "invite",
    { email: "erin@example.com", name: "" },
    invited(false)
  )
  assert.equal(
    (existingCalls[0].params as { name: unknown }).name,
    null,
    "an empty name is sent as none"
  )
  assert.equal(
    existing.message,
    "The existing account with erin@example.com is now a member."
  )
})

test("Deepwell refusals become readable messages", async () => {
  const cases: [string, Record<string, string>, unknown, number, string][] = [
    [
      "invite",
      { email: "new@example.com", name: "" },
      { code: 4109, message: "A name is required to create the account" },
      400,
      "No account uses that email address. Enter a name for the new account."
    ],
    [
      "invite",
      { email: "carol@example.com", name: "" },
      { code: 2109, message: "The user is already a member of the site" },
      400,
      "That account is already a member of this site."
    ],
    [
      "invite",
      { email: "new@example.com", name: "New" },
      { code: 0, message: "failed", data: { code_trace: [1001, 1305, 1210] } },
      500,
      "The invite email could not be sent. Nothing was changed."
    ],
    [
      "remove",
      { userId: "7", name: "OzmaAsimov" },
      { code: 3110, message: "The site's root member cannot be changed or removed" },
      400,
      "The site owner cannot be changed or removed."
    ],
    [
      "role",
      { userId: "61", name: "Carol", role: "admin" },
      { code: 3106, message: "Permission denied" },
      403,
      "Only administrators of this site can manage its members."
    ]
  ]
  for (const [action, fields, error, status, message] of cases) {
    const { value } = await submit(
      action,
      fields,
      responder((rpc) => (rpc.method.startsWith("member_admin_") ? { error } : undefined))
    )
    assert.equal(value.status, status, message)
    assert.equal(value.data?.message, message)
  }
})

test("actions without a session are refused before any backend call", async () => {
  for (const action of ["role", "remove", "invite"]) {
    const { value, calls } = await submit(
      action,
      { userId: "61", name: "Carol", role: "member", email: "x@example.com" },
      responder(),
      null
    )
    assert.equal(value.status, 401, action)
    assert.equal(calls.length, 0, action)
  }
})

function renderLoginStatus(siteAdmin: boolean) {
  const page = {
    url: new URL("http://local.test/start"),
    data: { user_session: { session: {}, user: alice }, site_admin: siteAdmin },
    form: null,
    params: {},
    route: { id: "/[slug]" },
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
  return render(LoginStatus, { context }).body
}

test("the header account menu links to the members page for site admins only", () => {
  assert.match(renderLoginStatus(true), /<a href="\/-\/admin\/members">Site members<\/a>/)
  assert.doesNotMatch(renderLoginStatus(false), /admin\/members|Site members/)
})
