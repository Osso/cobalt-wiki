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
const { uploadFiles, buildUploadFormData } = await vite.ssrLoadModule(
  "/src/lib/fileUploadBatch.ts"
)
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
  const snapshots: Array<Array<{ state: string; error?: string }>> = []
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
    onStatus: (rows: Array<{ state: string; error?: string }>) => snapshots.push(rows),
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
