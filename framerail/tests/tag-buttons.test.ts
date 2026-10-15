import assert from "node:assert/strict"
import { after, test } from "node:test"
import {
  applyTagChanges,
  splitTagInput,
  tagChangesBetween
} from "../src/lib/tag-buttons.ts"
import { createSsrServer } from "./ssr-server.ts"

// writing:_template's Publish button: [[button set-tags +_completed -@@ text="Publish"]]
const PUBLISH = "+_completed -@@"

test("Publish adds _completed and removes the literal @@ tag", () => {
  // character:corvin carried "@@" from an empty form field until Publish.
  assert.deepEqual(applyTagChanges(["@@", "alli", "corvin", "hunter"], PUBLISH), [
    "alli",
    "corvin",
    "hunter",
    "_completed"
  ])
})

test("Publish on a draft without @@ only adds _completed", () => {
  assert.deepEqual(applyTagChanges(["story", "rated-t"], PUBLISH), [
    "story",
    "rated-t",
    "_completed"
  ])
})

test("changes are idempotent and ignore words without a sign", () => {
  assert.deepEqual(applyTagChanges(["_completed"], `${PUBLISH} stray + -`), [
    "_completed"
  ])
})

test("changes apply in order", () => {
  assert.deepEqual(applyTagChanges(["a"], "-a +a -b +b -b"), ["a"])
})

test("the tag editor's edit becomes removals then additions", () => {
  assert.equal(tagChangesBetween(["a", "b", "c"], ["c", "d", "a"]), "-b +d")
  assert.equal(tagChangesBetween(["a"], ["a"]), "")
})

// writing:_template's copy box pastes the raw `characters` field, which members
// often write comma-separated; Wikidot split tags on spaces and commas.
const COPY_BOX = "Gabriella, Lericten,Mayellen  re:dara , re:zath"
const COPY_BOX_TAGS = ["Gabriella", "Lericten", "Mayellen", "re:dara", "re:zath"]

test("tag editor input splits on commas as well as whitespace", () => {
  assert.deepEqual(splitTagInput(COPY_BOX), COPY_BOX_TAGS)
  assert.deepEqual(splitTagInput(" , "), [])
})

const { vite, close } = await createSsrServer()
after(close)
const { actions } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.server.ts"
)

type Rpc = { method: string; params: unknown; id: string | number }
type Call = Rpc & { headers: Record<string, string> }
type Result = { tags?: string[]; status?: number; data?: { message: string } }

const PAGE = { page_id: 77, revision_id: 901, tags: ["@@", "story", "rated-t"] }

async function setTags(
  changes: string,
  responder: (rpc: Rpc) => unknown
): Promise<{ value: Result; calls: Call[] }> {
  const previousFetch = globalThis.fetch
  const calls: Call[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as Rpc
    calls.push({ ...rpc, headers: init?.headers as Record<string, string> })
    return new Response(
      JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...(responder(rpc) as object) })
    )
  }
  try {
    const value = await (actions.setTags as (event: unknown) => Promise<Result>)({
      request: new Request("http://local.test/writing:draft?/setTags", {
        method: "POST",
        headers: { "X-Wikijump-Site-Id": "6000000", "X-Wikijump-Site-Slug": "cobalt" },
        body: new URLSearchParams({ changes })
      }),
      params: { slug: "writing:draft" },
      cookies: { get: () => "session-secret" },
      getClientAddress: () => "203.0.113.9",
      locals: {
        requestContext: {
          sessionToken: "session-secret",
          siteId: 6000000,
          page: "writing:draft"
        }
      }
    })
    return { value, calls }
  } finally {
    globalThis.fetch = previousFetch
  }
}

function deepwell(canEdit: boolean, pageEdit: unknown = { result: null }) {
  return (rpc: Rpc) => {
    switch (rpc.method) {
      case "page_edit_permission":
        return { result: { can_edit: canEdit } }
      case "session_get":
        return { result: { user_id: 42, session_token: "session-secret" } }
      case "page_get":
        return { result: { ...PAGE, title: "Draft", slug: "writing:draft" } }
      case "page_edit":
        return pageEdit
    }
    throw new Error(`unexpected RPC ${rpc.method}`)
  }
}

test("editor's Publish edits only the tags of the current revision", async () => {
  const { value, calls } = await setTags(
    PUBLISH,
    deepwell(true, { result: { revision_id: 902, revision_number: 3 } })
  )
  assert.deepEqual(value, { tags: ["story", "rated-t", "_completed"] })
  assert.deepEqual(
    calls.map((call) => call.method),
    ["page_edit_permission", "session_get", "page_get", "page_edit"]
  )
  assert.deepEqual(calls[2].params, { site_id: 6000000, page: "writing:draft" })
  assert.deepEqual(calls[3].params, {
    site_id: 6000000,
    page: 77,
    user_id: 42,
    ip_address: "203.0.113.9",
    last_revision_id: 901,
    revision_comments: "",
    tags: ["story", "rated-t", "_completed"]
  })
  // Deepwell checks permission again against the trusted session and page.
  assert.equal(calls[3].headers["X-Deepwell-Session-Token"], "session-secret")
  assert.equal(calls[3].headers["X-Deepwell-Page"], "writing:draft")
})

test("page edit sends comma-separated tags as separate tags", async () => {
  const previousFetch = globalThis.fetch
  const calls: Rpc[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as Rpc
    calls.push(rpc)
    const result =
      rpc.method === "session_get"
        ? { user_id: 42, restricted: false }
        : { revision_id: 902, revision_number: 3 }
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, result }))
  }
  try {
    await actions.edit({
      request: new Request("http://local.test/writing:draft", {
        method: "POST",
        headers: { "X-Wikijump-Site-Id": "6000000", "X-Wikijump-Site-Slug": "cobalt" },
        body: new URLSearchParams({
          siteId: "6000000",
          pageId: "77",
          lastRevisionId: "901",
          title: "Draft",
          altTitle: "",
          wikitext: "Draft body",
          tags: COPY_BOX,
          comments: ""
        })
      }),
      params: { slug: "writing:draft", extra: "" },
      cookies: { get: () => "session-secret" },
      getClientAddress: () => "127.0.0.1",
      locals: {
        requestContext: {
          siteId: 6000000,
          page: "writing:draft",
          sessionToken: "session-secret"
        }
      }
    })
  } finally {
    globalThis.fetch = previousFetch
  }
  const edit = calls.find((call) => call.method === "page_edit")
  assert.deepEqual((edit?.params as { tags: string[] }).tags, COPY_BOX_TAGS)
})

test("a visitor without edit permission gets the Edit button's error and no edit", async () => {
  const { value, calls } = await setTags(PUBLISH, deepwell(false))
  assert.equal(value.status, 403)
  assert.equal(
    value.data?.message,
    "UNTRANSLATED:You don't have permission to edit this page"
  )
  assert.deepEqual(
    calls.map((call) => call.method),
    ["page_edit_permission"]
  )
})

test("Deepwell's refusal of the edit is shown, not swallowed", async () => {
  const { value } = await setTags(
    PUBLISH,
    deepwell(true, {
      error: {
        code: 4030,
        message: "user does not have permission to edit this page"
      }
    })
  )
  assert.equal(value.status, 400)
  assert.equal(value.data?.message, "user does not have permission to edit this page")
})
