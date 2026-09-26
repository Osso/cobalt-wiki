import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { pageFileEditAction, pageFileRollbackAction } = await vite.ssrLoadModule(
  "/src/lib/server/load/page.ts"
)

const identity = { siteId: 6000000, pageId: 314, fileId: 1474, lastRevisionId: 902 }

async function withRpc(action: () => Promise<unknown>) {
  const previousFetch = globalThis.fetch
  const calls: { method: string; params: Record<string, unknown> }[] = []
  globalThis.fetch = async (_input, init) => {
    const request = JSON.parse(String(init?.body))
    calls.push(request)
    const response =
      request.method === "session_get"
        ? { result: { user_id: 51 } }
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
    getClientAddress: () => ipAddress
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
