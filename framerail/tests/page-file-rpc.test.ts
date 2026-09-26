import assert from "node:assert/strict"
import { after, test } from "node:test"
const { stringify } = await import(
  new URL("../node_modules/.pnpm/node_modules/devalue/index.js", import.meta.url).href
)
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const pageActions = await vite.ssrLoadModule("/src/lib/server/load/page.ts")
const { pageFileEditAction, pageFileRollbackAction } = pageActions
const { actions } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.server.ts"
)

const identity = { siteId: 6000000, pageId: 314, fileId: 1474, lastRevisionId: 902 }

for (const selected of [[], [315, 317]]) {
  test(`Move serializes only selected dependency IDs: ${selected.join(",")}`, async () => {
    const request = new Request("http://local.test/page?/move", {
      method: "POST",
      body: new URLSearchParams({
        __superform_json: stringify({
          ...identity,
          newSlug: "new-page",
          comments: "Move",
          fixDependencies: selected
        })
      })
    })
    const { calls } = await withRpc(() =>
      actions.move(event(request, "127.0.0.1") as never)
    )
    const mutation = calls.find(({ method }) => method === "page_move")
    assert.ok(mutation)
    assert.deepEqual(mutation.params.fix_dependencies, selected)
  })
}

async function withRpc(action: () => Promise<unknown>, deny = false) {
  const previousFetch = globalThis.fetch
  const calls: { method: string; params: Record<string, unknown>; headers: Headers }[] =
    []
  globalThis.fetch = async (_input, init) => {
    if (init?.method === "PUT") return new Response(null, { status: 200 })
    const request = JSON.parse(String(init?.body))
    calls.push({ ...request, headers: new Headers(init?.headers) })
    const response =
      request.method === "session_get"
        ? { result: { user_id: 51 } }
        : request.method === "blob_upload"
          ? {
              result: {
                presign_url: "http://local.test/upload",
                pending_blob_id: "blob-1"
              }
            }
          : deny
            ? { error: { code: 3106, message: "Permission denied" } }
            : request.params.ip_address
              ? { result: { file_id: 1474, file_revision_id: 903 } }
              : { error: { code: -32602, message: "missing field `ip_address`" } }
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: request.id, ...response }), {
      headers: { "content-type": "application/json" }
    })
  }
  try {
    return { result: await action(), calls }
  } finally {
    globalThis.fetch = previousFetch
  }
}

function event(request: Request, ipAddress: string) {
  return {
    request,
    cookies: { get: () => "test-session" },
    getClientAddress: () => ipAddress,
    params: { slug: "source:page" },
    locals: {
      requestContext: {
        sessionToken: "test-session",
        siteId: 6000000,
        page: "source:page"
      }
    }
  }
}

test("file edit sends actual request IP in serialized RPC and accepts the revision", async () => {
  const fields = new URLSearchParams({
    ...Object.fromEntries(
      Object.entries(identity).map(([key, value]) => [key, String(value)])
    ),
    name: "renamed.txt",
    comments: "Rename"
  })
  const request = new Request("http://local.test/page?/fileEdit", {
    method: "POST",
    body: fields
  })
  const { result, calls } = await withRpc(() =>
    pageFileEditAction(event(request, "2001:db8::cafe") as never)
  )
  assert.deepEqual(
    calls.map(({ method }) => method),
    ["session_get", "file_edit"]
  )
  assert.deepEqual(calls[1].params, {
    site_id: 6000000,
    page_id: 314,
    user_id: 51,
    ip_address: "2001:db8::cafe",
    file_id: 1474,
    last_revision_id: 902,
    name: "renamed.txt",
    revision_comments: "Rename",
    bypass_filter: false
  })
  assert.deepEqual((result as { res: unknown }).res, {
    file_id: 1474,
    file_revision_id: 903
  })
})

test("page route dispatches file rollback to its RPC", async () => {
  const request = new Request("http://local.test/page?/fileRollback", {
    method: "POST",
    body: JSON.stringify({ ...identity, revisionNumber: 1, comments: "Restore bytes" })
  })
  const { result, calls } = await withRpc(() =>
    actions.fileRollback(event(request, "198.51.100.27") as never)
  )
  assert.deepEqual(
    calls.map(({ method }) => method),
    ["session_get", "file_rollback"]
  )
  assert.deepEqual((result as { res: unknown }).res, {
    file_id: 1474,
    file_revision_id: 903
  })
})

test("file rollback serializes omitted comments as an empty string", async () => {
  const request = new Request("http://local.test/page?/fileRollback", {
    method: "POST",
    body: JSON.stringify({ ...identity, revisionNumber: 1 })
  })
  const { calls } = await withRpc(() =>
    pageFileRollbackAction(event(request, "198.51.100.27") as never)
  )
  assert.deepEqual(
    calls.map(({ method }) => method),
    ["session_get", "file_rollback"]
  )
  assert.equal(calls[1].params.revision_comments, "")
})

test("file rollback preserves request IP in serialized RPC and accepts the revision", async () => {
  const request = new Request("http://local.test/page?/fileRollback", {
    method: "POST",
    body: JSON.stringify({ ...identity, revisionNumber: 1, comments: "Restore bytes" })
  })
  const { result, calls } = await withRpc(() =>
    pageFileRollbackAction(event(request, "198.51.100.27") as never)
  )
  assert.deepEqual(
    calls.map(({ method }) => method),
    ["session_get", "file_rollback"]
  )
  assert.deepEqual(calls[1].params, {
    site_id: 6000000,
    page_id: 314,
    user_id: 51,
    ip_address: "198.51.100.27",
    file: 1474,
    last_revision_id: 902,
    revision_number: 1,
    revision_comments: "Restore bytes",
    bypass_filter: false
  })
  assert.deepEqual((result as { res: unknown }).res, {
    file_id: 1474,
    file_revision_id: 903
  })
})

const mutationCases = [
  ["pageDeleteAction", "page_delete", { option: "delete", comments: "Delete" }],
  [
    "pageDeleteAction",
    "page_move",
    { option: "move", newSlug: "source:moved", comments: "Move" }
  ],
  ["pageMoveAction", "page_move", { newSlug: "source:moved", comments: "Move" }],
  [
    "pageParentSetAction",
    "parent_update",
    { parents: "", addParents: ["source:parent"] }
  ],
  ["pageRestoreAction", "page_restore", { comments: "Restore" }],
  ["pageFileDeleteAction", "file_delete", { ...identity, comments: "Delete" }],
  [
    "pageFileEditAction",
    "file_edit",
    { ...identity, name: "renamed.txt", comments: "Rename" }
  ],
  [
    "pageFileMoveAction",
    "file_move",
    { ...identity, destinationPage: "source:target", name: "", comments: "Move" }
  ],
  [
    "pageFileRestoreAction",
    "file_restore",
    { ...identity, newPage: "", newName: "", comments: "Restore" }
  ],
  [
    "pageFileRollbackAction",
    "file_rollback",
    { ...identity, revisionNumber: 1, comments: "Rollback" }
  ]
] as const

for (const [actionName, rpcMethod, extra] of mutationCases) {
  test(`${actionName} ${rpcMethod} forwards authenticated headers and returns permission-denied 403`, async () => {
    const fields = { ...identity, ...extra }
    const formAction = [
      "pageDeleteAction",
      "pageMoveAction",
      "pageParentSetAction",
      "pageRestoreAction",
      "pageFileEditAction",
      "pageFileMoveAction",
      "pageFileRestoreAction"
    ].includes(actionName)
    const body = formAction
      ? new URLSearchParams(
          Object.entries(fields).flatMap(([key, value]) =>
            Array.isArray(value)
              ? value.map((entry) => [key, String(entry)])
              : [[key, String(value)]]
          )
        )
      : JSON.stringify(fields)
    const request = new Request(`http://local.test/source:page?/${rpcMethod}`, {
      method: "POST",
      body
    })
    const { result, calls } = await withRpc(
      () => pageActions[actionName](event(request, "198.51.100.27") as never),
      true
    )
    const mutation = calls.find(({ method }) => method === rpcMethod)
    assert.ok(mutation, `${rpcMethod} sent`)
    assert.equal(mutation.headers.get("X-Deepwell-Session-Token"), "test-session")
    assert.equal(mutation.headers.get("X-Deepwell-Site-Id"), "6000000")
    assert.equal(mutation.headers.get("X-Deepwell-Page"), "source:page")
    assert.equal((result as { status: number }).status, 403)
  })
}

test("mutation uses session identity and request site instead of forged body identity", async () => {
  const request = new Request("http://local.test/source:page?/fileDelete", {
    method: "POST",
    body: JSON.stringify({
      ...identity,
      siteId: 6000001,
      userId: 999,
      comments: "Delete"
    })
  })
  const { calls } = await withRpc(
    () => pageActions.pageFileDeleteAction(event(request, "198.51.100.27") as never),
    true
  )
  const mutation = calls.find(({ method }) => method === "file_delete")
  assert.ok(mutation)
  assert.equal(mutation.params.user_id, 51)
  assert.equal(mutation.params.site_id, 6000001)
  assert.equal(mutation.headers.get("X-Deepwell-Site-Id"), "6000000")
  assert.equal(mutation.headers.get("X-Deepwell-Page"), "source:page")
})

test("file upload forwards authenticated context and returns permission-denied 403", async () => {
  const body = new FormData()
  for (const [key, value] of Object.entries(identity)) body.set(key, String(value))
  body.set("file", new File(["contents"], "report.txt"))
  body.set("name", "report.txt")
  body.set("comments", "Upload")
  const request = new Request("http://local.test/source:page?/fileUpload", {
    method: "POST",
    body
  })
  const { result, calls } = await withRpc(
    () => pageActions.pageFileUploadAction(event(request, "198.51.100.27") as never),
    true
  )
  const mutation = calls.find(({ method }) => method === "file_create")
  assert.ok(mutation)
  assert.equal(mutation.headers.get("X-Deepwell-Session-Token"), "test-session")
  assert.equal(mutation.headers.get("X-Deepwell-Site-Id"), "6000000")
  assert.equal(mutation.headers.get("X-Deepwell-Page"), "source:page")
  assert.equal((result as { status: number }).status, 403)
})
