import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.server.ts"
)
const { render } = await vite.ssrLoadModule("svelte/server")
const { pageLayoutState } = await vite.ssrLoadModule("/src/lib/stores.svelte.ts")
const { Layout } = await vite.ssrLoadModule("/src/lib/types.ts")

// Loaded per test so a missing module fails only the tests that need it.
async function component(name: string) {
  const module = await vite.ssrLoadModule(`/src/routes/[slug]/[...extra]/${name}.svelte`)
  return module.default
}

/** Collapses whitespace as the browser does when displaying text. */
async function renderText(name: string, props: Record<string, unknown>) {
  return render(await component(name), { props }).body.replace(/\s+/g, " ")
}

const HEADERS = { "X-Wikijump-Site-Id": "6000000", "X-Wikijump-Site-Slug": "cobalt" }
const PANE_TEXT =
  "Changes the page URL, tags, writing/arc/chain name lists, relationship boxes, links and icon. The display name is not changed."
const preview = {
  old_name: "atley",
  new_name: "atly",
  old_slug: "character:atley",
  new_slug: "character:atly",
  preview_token: "snapshot-token",
  icon: true,
  destination_tag_pages: 0,
  count: 2,
  pages: [
    { page_id: 77, slug: "character:atley", title: "Atley Vance" },
    { page_id: 81, slug: "writing:first-light", title: "First <Light>" }
  ]
}
const progress = {
  operation_id: 5,
  status: "running",
  done: 1,
  total: 2,
  error: null,
  new_slug: "character:atly"
}

type Rpc = { method: string; params: Record<string, unknown>; id: number }
type Call = Rpc & { headers: Record<string, string> }
type ActionResult = {
  preview?: typeof preview
  operationId?: number
  progress?: typeof progress
  status?: number
  data?: { message: string }
}

async function submit(
  action: string,
  fields: Record<string, string>,
  reply: (rpc: Rpc) => object
) {
  const previousFetch = globalThis.fetch
  const calls: Call[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as Rpc
    calls.push({ ...rpc, headers: init?.headers as Record<string, string> })
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...reply(rpc) }))
  }
  try {
    const value = await (actions[action] as (event: unknown) => Promise<ActionResult>)({
      request: new Request("http://local.test/character:atley", {
        method: "POST",
        headers: HEADERS,
        body: new URLSearchParams(fields)
      }),
      params: { slug: "character:atley", extra: "" },
      cookies: { get: () => "session-secret" },
      getClientAddress: () => "203.0.113.9",
      locals: {
        requestContext: {
          sessionToken: "session-secret",
          siteId: 6000000,
          page: "character:atley"
        }
      }
    })
    return { value, calls: calls.filter((c) => c.method.startsWith("character_rename_")) }
  } finally {
    globalThis.fetch = previousFetch
  }
}

const unreachable = (rpc: Rpc) => {
  throw new Error(`unexpected RPC ${rpc.method}`)
}
const startFields = {
  pageId: "77",
  newName: "atly",
  previewToken: "snapshot-token",
  confirmed: "yes"
}

test("Rename character is offered on character and bgc pages, never their templates", async () => {
  const { isCharacterPage } = await vite.ssrLoadModule("/src/lib/character-rename.ts")
  for (const slug of ["character:atley", "bgc:old-tom"]) {
    assert.equal(isCharacterPage(slug), true, slug)
  }
  for (const slug of [
    "character:_template",
    "bgc:_template",
    "character:",
    "writing:atley",
    "characters:atley",
    "atley"
  ]) {
    assert.equal(isCharacterPage(slug), false, slug)
  }
})

test("pane asks for the new short name and explains what changes", async () => {
  pageLayoutState.current = Layout.WIKIDOT
  const body = await renderText("CharacterRenamePane", {
    data: { page: { page_id: 77, slug: "character:atley" } },
    close() {}
  })
  assert.match(body, /<h1[^>]*>Rename character<\/h1>/)
  assert.ok(body.includes(PANE_TEXT))
  assert.match(body, /<input[^>]*name="newName"/)
  assert.match(body, /value="Preview"/)
  assert.match(body, /value="Cancel"/)
  assert.doesNotMatch(body, /Confirm rename/)
})

test("confirmation step lists the slug change, every affected page and the icon", async () => {
  const body = await renderText("CharacterRenameConfirm", {
    preview,
    busy: false,
    confirm() {},
    cancel() {}
  })
  assert.match(body, /character:atley → character:atly/)
  assert.match(body, /2 pages/)
  assert.match(body, /Atley Vance/)
  assert.match(body, /First &lt;Light(?:>|&gt;)/)
  assert.match(body, /writing:first-light/)
  assert.match(body, /icon file will be renamed/i)
  assert.match(body, /value="Confirm rename"/)
  assert.match(body, /value="Cancel"/)

  const single = await renderText("CharacterRenameConfirm", {
    preview: { ...preview, icon: false, count: 1, pages: [preview.pages[0]] },
    busy: false,
    confirm() {},
    cancel() {}
  })
  assert.match(single, /1 page\b/)
  assert.match(single, /no icon file/i)
  assert.doesNotMatch(single, /first-light/)
})

test("destination tags need an informative explicit confirmation", async () => {
  const body = await renderText("CharacterRenameConfirm", {
    preview: { ...preview, destination_tag_pages: 20 },
    busy: false,
    confirm() {},
    cancel() {}
  })
  assert.match(body, /20 pages already use/)
  assert.match(body, /atly/)
  assert.match(body, /re:atly/)
  assert.match(body, /Existing tags will be kept/)
  assert.match(body, /value="Confirm rename and keep existing tags"/)
})

test("destination-tag acknowledgment is forwarded only when explicitly accepted", async () => {
  for (const [allowExistingTags, expected] of [
    ["yes", true],
    ["no", false],
    ["", false]
  ] as const) {
    const { calls } = await submit(
      "characterRenameStart",
      { ...startFields, allowExistingTags },
      () => ({ result: { operation_id: 5 } })
    )
    assert.equal(calls[0].params.allow_existing_tags, expected)
  }
})

test("preview forwards the page and trimmed name with the page's request context", async () => {
  const { value, calls } = await submit(
    "characterRenamePreview",
    { pageId: "77", newName: " atly " },
    () => ({ result: preview })
  )
  assert.deepEqual(value, { preview })
  assert.deepEqual(
    calls.map((c) => [c.method, c.params]),
    [["character_rename_preview", { page_id: 77, new_name: "atly" }]]
  )
  assert.equal(calls[0].headers["X-Deepwell-Session-Token"], "session-secret")
  assert.equal(calls[0].headers["X-Deepwell-Site-Id"], "6000000")
  assert.equal(calls[0].headers["X-Deepwell-Page"], "character:atley")
})

test("blank names and invalid page IDs never reach Deepwell", async () => {
  for (const fields of [
    { pageId: "77", newName: "   " },
    { pageId: "", newName: "atly" },
    { pageId: "0", newName: "atly" },
    { pageId: "abc", newName: "atly" }
  ]) {
    const { value, calls } = await submit("characterRenamePreview", fields, unreachable)
    assert.equal(value.status, 400, JSON.stringify(fields))
    assert.equal(calls.length, 0)
  }
})

test("start requires explicit confirmation and the preview token", async () => {
  for (const fields of [
    { ...startFields, confirmed: "" },
    { ...startFields, confirmed: "no" },
    { ...startFields, previewToken: "" }
  ]) {
    const { value, calls } = await submit("characterRenameStart", fields, unreachable)
    assert.equal(value.status, 400, JSON.stringify(fields))
    assert.match(value.data!.message, /preview/i)
    assert.equal(calls.length, 0)
  }
  const { value, calls } = await submit("characterRenameStart", startFields, () => ({
    result: { operation_id: 5 }
  }))
  assert.deepEqual(value, { operationId: 5 })
  assert.deepEqual(
    calls.map((c) => [c.method, c.params]),
    [
      [
        "character_rename_start",
        {
          page_id: 77,
          new_name: "atly",
          preview_token: "snapshot-token",
          allow_existing_tags: false,
          ip_address: "203.0.113.9"
        }
      ]
    ]
  )
  assert.equal(calls[0].headers["X-Deepwell-Session-Token"], "session-secret")
})

test("status forwards the operation ID and returns progress", async () => {
  const { value, calls } = await submit(
    "characterRenameStatus",
    { operationId: "5" },
    () => ({ result: progress })
  )
  assert.deepEqual(value, { progress })
  assert.deepEqual(
    calls.map((c) => [c.method, c.params]),
    [["character_rename_status", { operation_id: 5 }]]
  )
  const invalid = await submit("characterRenameStatus", { operationId: "" }, unreachable)
  assert.equal(invalid.value.status, 400)
  assert.equal(invalid.calls.length, 0)
})

test("Deepwell refusals become rename messages with matching HTTP status", async () => {
  const cases: [number, string, number, string][] = [
    [
      3106,
      "Permission denied",
      403,
      "You don't have permission to rename this character or one of the pages that mention it"
    ],
    [4000, "Not a character page", 400, "Not a character page"],
    [2104, "Page slug exists", 400, "A character page with that name already exists"],
    [2105, "File exists", 400, "An icon with that name already exists"],
    [
      4306,
      "Page not editable",
      400,
      "One of the affected pages is locked or not editable"
    ],
    [4304, "Preview stale", 400, "Affected pages changed; preview again"],
    [
      4307,
      "Name in use",
      400,
      "Preview again and confirm keeping existing destination tags"
    ]
  ]
  for (const [code, backendMessage, status, message] of cases) {
    const { value } = await submit("characterRenameStart", startFields, () => ({
      error: { code, message: backendMessage }
    }))
    assert.equal(value.status, status, String(code))
    assert.equal(value.data?.message, message)
  }
  const denied = await submit(
    "characterRenamePreview",
    { pageId: "77", newName: "atly" },
    () => ({ error: { code: 3106, message: "Permission denied" } })
  )
  assert.equal(denied.value.status, 403)
})
