import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createRequire } from "node:module"
import { realpathSync } from "node:fs"
import { createSsrServer } from "./ssr-server.ts"

const kitRequire = createRequire(
  realpathSync(
    new URL("../node_modules/@sveltejs/kit/src/runtime/app/forms.js", import.meta.url)
  )
)
const { stringify } = await import(kitRequire.resolve("devalue"))
const { vite, close } = await createSsrServer()
after(close)
const { render } = await vite.ssrLoadModule("svelte/server")
const kitAppPath = realpathSync(
  new URL("../node_modules/@sveltejs/kit/src/runtime/server/app.js", import.meta.url)
)
const { set_app } = await vite.ssrLoadModule(kitAppPath)
set_app({ decoders: {} })
const { uploadFiles, buildUploadFormData, buildReplaceFormData, replaceFile } =
  await vite.ssrLoadModule("/src/lib/fileUploadBatch.ts")
const { default: FileUploadForm } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/FileUploadForm.svelte"
)

const files = [
  new File(["first bytes"], "one.txt", { type: "text/plain" }),
  new File(["second bytes"], "two.txt", { type: "text/plain" }),
  new File(["third bytes"], "three.txt", { type: "text/plain" })
]
const identity = { siteId: 6000000, pageId: 314, lastRevisionId: 902 }

function envelope(type: "success" | "failure", data: Record<string, unknown>) {
  return JSON.stringify({
    type,
    status: type === "success" ? 200 : 409,
    data: stringify(data)
  })
}

test("multipart payload carries actual bytes, page identity, actual revision and original batch name", async () => {
  const form = buildUploadFormData(files[1], identity, "", "Shared comment")
  assert.deepEqual(
    [...form.keys()],
    ["siteId", "pageId", "lastRevisionId", "file", "name", "comments"]
  )
  assert.equal(form.get("siteId"), "6000000")
  assert.equal(form.get("pageId"), "314")
  assert.equal(form.get("lastRevisionId"), "902")
  const sentFile = form.get("file")
  assert.ok(sentFile instanceof File)
  assert.equal(sentFile.name, "two.txt")
  assert.equal(await sentFile.text(), "second bytes")
  assert.equal(form.get("name"), "")
  assert.equal(form.get("comments"), "Shared comment")
  assert.equal(
    buildUploadFormData(files[0], identity, "Renamed.txt", "").get("name"),
    "Renamed.txt"
  )
})

test("three sequential uploads retain success around middle action failure and refresh once", async () => {
  const calls: { url: string; init: RequestInit }[] = []
  const snapshots: { state: string; error?: string }[][] = []
  let outstanding = 0
  let refreshes = 0
  const responses = [
    envelope("success", { res: { file_id: 1 } }),
    envelope("failure", { message: "File already exists", code: "conflict" }),
    envelope("success", { res: { file_id: 3 } })
  ]
  const fetchUpload = async (url: string, init: RequestInit) => {
    assert.equal(outstanding++, 0)
    calls.push({ url, init })
    const response = new Response(responses[calls.length - 1], { status: 200 })
    outstanding--
    return response
  }
  const results = await uploadFiles({
    files,
    identity,
    name: "Ignored for batch",
    comments: "Same comment",
    url: "https://wiki.example/source:page?/fileUpload",
    fetchUpload,
    isActive: () => true,
    onStatus: (rows: { state: string; error?: string }[]) => snapshots.push(rows),
    onUploaded: async () => {
      refreshes++
    }
  })
  assert.deepEqual(
    results.map((row: { state: string }) => row.state),
    ["uploaded", "failed", "uploaded"]
  )
  assert.equal(results[1].error, "File already exists")
  assert.equal(refreshes, 1)
  assert.equal(calls.length, 3)
  assert.ok(
    snapshots.some(
      (rows) => rows.map((row) => row.state).join() === "uploaded,failed,queued"
    )
  )
  for (const [index, call] of calls.entries()) {
    assert.equal(call.url, "https://wiki.example/source:page?/fileUpload")
    assert.equal(call.init.method, "POST")
    const body = call.init.body
    assert.ok(body instanceof FormData)
    assert.equal(body.get("name"), files[index].name)
    assert.equal(body.get("comments"), "Same comment")
    assert.equal(body.get("siteId"), "6000000")
    assert.equal(body.get("pageId"), "314")
    assert.equal(body.get("lastRevisionId"), "902")
    const file = body.get("file")
    assert.ok(file instanceof File)
    assert.equal(await file.text(), await files[index].text())
  }
})

test("unmount stops unsent uploads without aborting in-flight request; successful upload still refreshes", async () => {
  let active = true
  let calls = 0
  let refreshes = 0
  const results = await uploadFiles({
    files,
    identity,
    name: "",
    comments: "",
    url: "https://wiki.example/source:page?/fileUpload",
    fetchUpload: async () => {
      calls++
      active = false
      return new Response(envelope("success", { res: { file_id: 1 } }))
    },
    isActive: () => active,
    onStatus: () => {},
    onUploaded: async () => {
      refreshes++
    }
  })
  assert.equal(calls, 1)
  assert.equal(refreshes, 1)
  assert.deepEqual(
    results.map((row: { state: string }) => row.state),
    ["uploaded", "queued", "queued"]
  )
})

test("validation errors and transport errors remain per-file; zero success does not refresh", async () => {
  let refreshed = false
  let calls = 0
  const results = await uploadFiles({
    files: files.slice(0, 2),
    identity,
    name: "",
    comments: "",
    url: "https://wiki.example/source:page?/fileUpload",
    fetchUpload: async () => {
      calls++
      if (calls === 2) throw new Error("Network unavailable")
      return new Response(
        envelope("failure", { form: { errors: { file: ["File too large"] } } })
      )
    },
    isActive: () => true,
    onStatus: () => {},
    onUploaded: async () => {
      refreshed = true
    }
  })
  assert.deepEqual(
    results.map((row: { error: string | undefined }) => row.error),
    ["File too large", "Network unavailable"]
  )
  assert.equal(refreshed, false)
})

test("empty transport errors and missing file receipts are not reported as uploads", async () => {
  let calls = 0
  let refreshed = false
  const results = await uploadFiles({
    files: files.slice(0, 2),
    identity,
    name: "",
    comments: "",
    url: "https://wiki.example/source:page?/fileUpload",
    fetchUpload: async () => {
      if (++calls === 1) throw new Error("")
      return new Response(envelope("success", { res: {} }))
    },
    isActive: () => true,
    onStatus: () => {},
    onUploaded: async () => {
      refreshed = true
    }
  })
  assert.deepEqual(
    results.map((row: { state: string }) => row.state),
    ["failed", "failed"]
  )
  assert.equal(refreshed, false)
})

test("error responses without an action status preserve the server error", async () => {
  const results = await uploadFiles({
    files: files.slice(0, 1),
    identity,
    name: "",
    comments: "",
    url: "https://wiki.example/source:page?/fileUpload",
    fetchUpload: async () =>
      new Response(
        JSON.stringify({ type: "error", error: { message: "Service unavailable" } }),
        { status: 503 }
      ),
    isActive: () => true,
    onStatus: () => {},
    onUploaded: async () => assert.fail("failed uploads cannot refresh as success")
  })
  assert.equal(results[0].state, "failed")
  assert.equal(results[0].error, "Service unavailable")
})

test("upload form retains Files layout hooks and multiple file selection", () => {
  const body = render(FileUploadForm, {
    props: {
      data: {
        site: { site_id: 6000000 },
        page: { page_id: 314 },
        page_revision: { revision_id: 902 },
        internationalization: { upload: "Upload", cancel: "Cancel" }
      },
      onUploaded: async () => {},
      onClose: () => {}
    }
  }).body
  assert.match(body, /id="file-upload"/)
  assert.match(body, /name="file"[^>]*multiple|multiple[^>]*name="file"/)
  assert.match(body, /name="name"/)
  assert.match(body, /name="comments"/)
})

const existing = { file_id: 1498, revision_id: 7741, name: "notes.txt" }

test("replacing a collision edits the existing file with its revision, name and new bytes", async () => {
  const upload = new File(["new notes"], "notes.txt", { type: "text/plain" })
  const form = buildReplaceFormData(upload, identity, existing, "Updated notes")
  assert.deepEqual(
    [...form.keys()],
    ["siteId", "pageId", "lastRevisionId", "fileId", "file", "name", "comments"]
  )
  assert.equal(form.get("siteId"), "6000000")
  assert.equal(form.get("pageId"), "314")
  assert.equal(form.get("lastRevisionId"), "7741")
  assert.equal(form.get("fileId"), "1498")
  assert.equal(form.get("name"), "notes.txt")
  assert.equal(form.get("comments"), "Updated notes")
  const sent = form.get("file")
  assert.ok(sent instanceof File)
  assert.equal(await sent.text(), "new notes")
})

test("replace succeeds on a new revision or identical bytes and fails without a receipt", async () => {
  const outcome = async (body: string) => {
    const calls: string[] = []
    const result = await replaceFile({
      file: files[0],
      existing,
      identity,
      comments: "",
      url: "https://wiki.example/source:page?/fileEdit",
      fetchUpload: async (url: string) => {
        calls.push(url)
        return new Response(body)
      }
    })
    assert.deepEqual(calls, ["https://wiki.example/source:page?/fileEdit"])
    return result
  }
  assert.deepEqual(
    await outcome(
      envelope("success", { res: { file_revision_id: 9, file_revision_number: 2 } })
    ),
    { state: "replaced" }
  )
  assert.deepEqual(await outcome(envelope("success", { res: null })), {
    state: "replaced"
  })
  assert.equal((await outcome(envelope("success", { res: {} }))).state, "failed")
  assert.deepEqual(
    await outcome(envelope("failure", { message: "File was changed by someone else" })),
    { state: "failed", error: "File was changed by someone else" }
  )
})

test("batch rows record the stored name used to find a colliding file", async () => {
  const [single] = await uploadFiles({
    files: files.slice(0, 1),
    identity,
    name: "Renamed.txt",
    comments: "",
    url: "https://wiki.example/source:page?/fileUpload",
    fetchUpload: async () => new Response(envelope("failure", { message: "exists" })),
    isActive: () => true,
    onStatus: () => {},
    onUploaded: async () => {}
  })
  assert.equal(single.name, "Renamed.txt")
  const batch = await uploadFiles({
    files: files.slice(0, 2),
    identity,
    name: "Ignored for batch",
    comments: "",
    url: "https://wiki.example/source:page?/fileUpload",
    fetchUpload: async () => new Response(envelope("failure", { message: "exists" })),
    isActive: () => true,
    onStatus: () => {},
    onUploaded: async () => {}
  })
  assert.deepEqual(
    batch.map((row: { name: string }) => row.name),
    ["one.txt", "two.txt"]
  )
})
