import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"
import type { TagPreview } from "../src/lib/server/deepwell/tag-admin.ts"
import type { loadTagAdminPage } from "../src/lib/server/load/tag-admin.ts"

const { vite, close } = await createSsrServer()
after(close)
const { load, actions } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/admin/tags/+page.server.ts"
)
const { default: TagAdmin } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/admin/tags/+page.svelte"
)
const { render } = await vite.ssrLoadModule("svelte/server")
const HEADERS = { "X-Wikijump-Site-Id": "6000011", "X-Wikijump-Site-Slug": "cobalt" }
const preview = {
  source_tag: "old",
  target_tag: "new",
  preview_token: "snapshot-token",
  count: 2,
  pages: [
    { page_id: 11, slug: "character:one", title: "One" },
    { page_id: 12, slug: "character:two", title: "<Two>" }
  ]
}
const operation = {
  operation_id: 31,
  source_tag: "old",
  target_tag: "new",
  count: 2,
  actor_user_id: 42,
  created_at: "2026-09-28T12:00:00Z",
  reversed: false,
  pages: preview.pages
}
type Rpc = { method: string; params: Record<string, unknown>; id: number }
type Call = Rpc & { headers: Record<string, string> }
type Loaded = Awaited<ReturnType<typeof loadTagAdminPage>>
type ActionResponse = {
  saved?: boolean
  preview?: TagPreview | null
  status?: number
  data?: { message: string }
}
async function withRpc<T>(reply: (rpc: Rpc) => object, invoke: () => Promise<T>) {
  const previous = globalThis.fetch
  const calls: Call[] = []
  globalThis.fetch = async (_url, init) => {
    const rpc = JSON.parse(String(init?.body)) as Rpc
    calls.push({ ...rpc, headers: init?.headers as Record<string, string> })
    const response =
      rpc.method === "translate"
        ? { result: {} }
        : rpc.method === "page_view"
          ? { result: { type: "missing", data: {} } }
          : reply(rpc)
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...response }))
  }
  try {
    return { value: await invoke(), calls }
  } finally {
    globalThis.fetch = previous
  }
}
function loadPage(token: string | null, reply: (rpc: Rpc) => object) {
  return withRpc(reply, () =>
    (load as (event: unknown) => Promise<Loaded>)({
      request: new Request("http://local.test/-/admin/tags", { headers: HEADERS }),
      cookies: { get: () => token },
      parent: async () => ({
        locales: ["en"],
        license_name: "CC",
        license_url: "https://example.test/license",
        user_session: token ? { user: { user_id: 42 } } : null
      })
    })
  )
}
function submit(
  action: string,
  fields: Record<string, string>,
  reply: (rpc: Rpc) => object,
  token: string | null = "session-secret"
) {
  return withRpc(reply, () =>
    (actions[action] as (event: unknown) => Promise<ActionResponse>)({
      request: new Request("http://local.test/-/admin/tags", {
        method: "POST",
        headers: HEADERS,
        body: new URLSearchParams(fields)
      }),
      cookies: { get: () => token },
      getClientAddress: () => "203.0.113.9"
    })
  )
}
const deny = () => ({ error: { code: 3106, message: "Permission denied" } })
const tagCalls = (calls: Call[]) => calls.filter((c) => c.method.startsWith("site_tag_"))

test("admin history uses trusted site and session headers; visitors cannot see operations", async () => {
  const admin = await loadPage("session-secret", () => ({ result: [operation] }))
  assert.equal(admin.value.access, "admin")
  assert.deepEqual(admin.value.operations, [operation])
  const [call] = tagCalls(admin.calls)
  assert.equal(call.method, "site_tag_history")
  assert.equal(call.headers["X-Deepwell-Site-Id"], "6000011")
  assert.equal(call.headers["X-Deepwell-Session-Token"], "session-secret")
  const guest = await loadPage(null, deny)
  assert.equal(guest.value.access, "signed-out")
  assert.equal(tagCalls(guest.calls).length, 0)
  const denied = await loadPage("member-session", deny)
  assert.equal(denied.value.access, "denied")
  assert.deepEqual(denied.value.operations, [])
})

test("preview names exact affected pages and never applies a change", async () => {
  const { value, calls } = await submit(
    "preview",
    { sourceTag: "old", targetTag: "new", operation: "rename" },
    () => ({ result: preview })
  )
  assert.deepEqual(value.preview, preview)
  assert.deepEqual(
    tagCalls(calls).map((c) => [c.method, c.params]),
    [["site_tag_preview", { source_tag: "old", target_tag: "new" }]]
  )
})

test("delete preview has no destination; invalid inputs never reach backend", async () => {
  const result = await submit(
    "preview",
    { sourceTag: "old", targetTag: "ignored", operation: "delete" },
    () => ({ result: { ...preview, target_tag: null } })
  )
  assert.deepEqual(tagCalls(result.calls)[0].params, {
    source_tag: "old",
    target_tag: null
  })
  for (const fields of [
    { sourceTag: "", targetTag: "new", operation: "rename" },
    { sourceTag: "old", targetTag: "", operation: "rename" },
    { sourceTag: "old", targetTag: "old", operation: "rename" },
    { sourceTag: "two tags", targetTag: "new", operation: "rename" }
  ]) {
    const invalid = await submit("preview", fields, deny)
    assert.equal(invalid.value.status, 400)
    assert.equal(tagCalls(invalid.calls).length, 0)
  }
})

test("apply requires confirmation and snapshot; actor and site cannot be supplied by form", async () => {
  const fields = {
    sourceTag: "old",
    targetTag: "new",
    operation: "rename",
    previewToken: "snapshot-token",
    confirmed: "yes",
    user_id: "99",
    site_id: "99"
  }
  for (const missing of [
    { ...fields, confirmed: "" },
    { ...fields, previewToken: "" }
  ]) {
    const result = await submit("apply", missing, deny)
    assert.equal(result.value.status, 400)
    assert.equal(tagCalls(result.calls).length, 0)
  }
  const result = await submit("apply", fields, () => ({ result: operation }))
  assert.equal(result.value.saved, true)
  assert.deepEqual(tagCalls(result.calls)[0].params, {
    source_tag: "old",
    target_tag: "new",
    preview_token: "snapshot-token",
    ip_address: "203.0.113.9"
  })
  assert.equal(result.value.preview, null)
})

test("denial and stale preview are visible failures, not successful mutations", async () => {
  const fields = {
    sourceTag: "old",
    operation: "delete",
    previewToken: "snapshot-token",
    confirmed: "yes"
  }
  const denied = await submit("apply", fields, deny)
  assert.equal(denied.value.status, 403)
  const guest = await submit("apply", fields, deny, null)
  assert.equal(guest.value.status, 401)
  assert.equal(tagCalls(guest.calls).length, 0)
  const stale = await submit("apply", fields, () => ({
    error: { code: 4304, message: "Preview changed; preview again" }
  }))
  assert.equal(stale.value.status, 400)
  assert.ok(stale.value.data)
  assert.match(stale.value.data.message, /preview/i)
})

test("undo requires explicit confirmation and uses operation ID only", async () => {
  const invalid = await submit("undo", { operationId: "31" }, deny)
  assert.equal(invalid.value.status, 400)
  assert.equal(tagCalls(invalid.calls).length, 0)
  const result = await submit("undo", { operationId: "31", confirmed: "yes" }, () => ({
    result: { ...operation, reversed: true }
  }))
  assert.equal(result.value.saved, true)
  assert.deepEqual(tagCalls(result.calls)[0].params, {
    operation_id: 31,
    ip_address: "203.0.113.9"
  })
})

test("rendered preview escapes names, shows count and confirmation; denied UI has no mutation forms", () => {
  const data = { access: "admin", operations: [operation] }
  const body = render(TagAdmin, { props: { data, form: { preview } } }).body
  assert.match(body, /2 pages/)
  assert.match(body, /character:one/)
  assert.match(body, /&lt;Two(?:>|&gt;)/)
  assert.match(body, /Confirm rename/)
  assert.match(body, /Undo/)
  const denied = render(TagAdmin, {
    props: { data: { access: "denied", operations: [] }, form: null }
  }).body
  assert.doesNotMatch(denied, /<form/)
})
