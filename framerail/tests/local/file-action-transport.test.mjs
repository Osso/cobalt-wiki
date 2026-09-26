import assert from "node:assert/strict"
import { test } from "node:test"
import {
  assertActionSuccess,
  guardBrowserWrites,
  mutationMatches,
  origin,
  siteId
} from "./file-action-transport.mjs"
const { stringify } = await import(
  new URL("../../node_modules/.pnpm/node_modules/devalue/index.js", import.meta.url).href
)

function nativeUploadForm() {
  const form = new FormData()
  form.set("file", new File(["proof"], "ui-file-run.txt"))
  form.set("name", "ui-file-run.txt")
  form.set("comments", "upload proof")
  form.set("siteId", String(siteId))
  form.set("pageId", "3000006134")
  form.set("lastRevisionId", "42")
  return form
}

async function formRequest(
  form,
  url = `${origin}/local-action-proof:source?/fileUpload`
) {
  const body = new Request(url, { method: "POST", body: form })
  const bytes = Buffer.from(await body.arrayBuffer())
  return {
    url: () => url,
    headers: () => Object.fromEntries(body.headers),
    postDataBuffer: () => bytes
  }
}

test("native fileUpload accepts exact fields and fixture identity", async () => {
  const request = await formRequest(nativeUploadForm())
  const permitted = { fileId: null, name: "ui-file-run.txt" }
  assert.equal(await mutationMatches(request, permitted, 3000006134), true)
  assert.equal(
    await mutationMatches(request, { ...permitted, name: "other.txt" }, 3000006134),
    false
  )
  assert.equal(await mutationMatches(request, permitted, 3000006135), false)
})

test("native fileUpload rejects wrong identity and noncanonical ID strings", async () => {
  const permitted = { fileId: null, name: "ui-file-run.txt" }
  for (const [field, value] of [
    ["siteId", "6000001"],
    ["siteId", "06000000"],
    ["pageId", "3000006135"],
    ["pageId", "03000006134"]
  ]) {
    const form = nativeUploadForm()
    form.set(field, value)
    assert.equal(
      await mutationMatches(await formRequest(form), permitted, 3000006134),
      false
    )
  }
})

test("native fileUpload rejects duplicate, missing, extra, or wrongly typed fields", async () => {
  const permitted = { fileId: null, name: "ui-file-run.txt" }
  const mutations = [
    (form) => form.append("siteId", String(siteId)),
    (form) => form.append("file", new File(["extra"], "ui-file-run.txt")),
    (form) => form.delete("comments"),
    (form) => form.set("fileId", "87"),
    (form) => form.set("extra", "value"),
    (form) => form.set("file", "ui-file-run.txt"),
    (form) => form.set("comments", new File(["extra"], "comment.txt")),
    (form) => form.set("name", "other.txt")
  ]
  for (const mutate of mutations) {
    const form = nativeUploadForm()
    mutate(form)
    await assert.rejects(mutationMatches(await formRequest(form), permitted, 3000006134))
  }
})

test("native fields are not accepted for other actions", async () => {
  const request = await formRequest(
    nativeUploadForm(),
    `${origin}/local-action-proof:source?/fileEdit`
  )
  await assert.rejects(
    mutationMatches(request, { fileId: null, name: "ui-file-run.txt" }, 3000006134),
    /stage=superform-json; contentType=multipart\/form-data/
  )
})

test("multipart upload authorizes only matching fixture and uploaded filename", async () => {
  const form = new FormData()
  form.set(
    "__superform_json",
    stringify({ siteId, pageId: 3000006134, name: "ui-file-run.txt", file: {} })
  )
  form.set("__superform_file_file", new File(["proof"], "ui-file-run.txt"))
  const body = new Request(origin, { method: "POST", body: form })
  const bytes = Buffer.from(await body.arrayBuffer())
  const request = {
    url: () => origin,
    headers: () => Object.fromEntries(body.headers),
    postDataBuffer: () => bytes
  }
  const permitted = { fileId: null, name: "ui-file-run.txt" }
  assert.equal(await mutationMatches(request, permitted, 3000006134), true)
  assert.equal(
    await mutationMatches(request, { ...permitted, name: "other.txt" }, 3000006134),
    false
  )
  assert.equal(await mutationMatches(request, permitted, 3000006135), false)
})

test("multipart edit authorizes devalue-encoded file identity", async () => {
  const form = new FormData()
  form.set("__superform_json", stringify({ siteId, pageId: 3000006134, fileId: 87 }))
  const body = new Request(origin, { method: "POST", body: form })
  const bytes = Buffer.from(await body.arrayBuffer())
  const request = {
    url: () => origin,
    headers: () => Object.fromEntries(body.headers),
    postDataBuffer: () => bytes
  }
  assert.equal(
    await mutationMatches(request, { fileId: 87, name: null }, 3000006134),
    true
  )
  assert.equal(
    await mutationMatches(request, { fileId: 78, name: null }, 3000006134),
    false
  )
  assert.equal(
    await mutationMatches(request, { fileId: 87, name: null }, 3000006135),
    false
  )
})

test("URL-encoded Superforms mutations require exact numeric identities", async () => {
  const form = new URLSearchParams({
    __superform_json: stringify({ siteId, pageId: 3000006134, fileId: 87 })
  })
  const body = new Request(origin, { method: "POST", body: form })
  const bytes = Buffer.from(await body.arrayBuffer())
  const request = {
    url: () => origin,
    headers: () => Object.fromEntries(body.headers),
    postDataBuffer: () => bytes
  }
  assert.equal(
    await mutationMatches(request, { fileId: 87, name: null }, 3000006134),
    true
  )
  assert.equal(
    await mutationMatches(request, { fileId: 78, name: null }, 3000006134),
    false
  )
  assert.equal(
    await mutationMatches(request, { fileId: 87, name: null }, 3000006135),
    false
  )
  for (const identity of [
    { siteId: String(siteId), pageId: 3000006134, fileId: 87 },
    { siteId, pageId: "3000006134", fileId: 87 },
    { siteId, pageId: 3000006134, fileId: "87" }
  ]) {
    form.set("__superform_json", stringify(identity))
    const stringBody = new Request(origin, { method: "POST", body: form })
    const stringBytes = Buffer.from(await stringBody.arrayBuffer())
    assert.equal(
      await mutationMatches(
        { ...request, postDataBuffer: () => stringBytes },
        { fileId: 87, name: null },
        3000006134
      ),
      false
    )
  }
})

test("URL-encoded Superforms diagnostics classify missing payload safely", async () => {
  const body = new Request(origin, { method: "POST", body: new URLSearchParams() })
  const bytes = Buffer.from(await body.arrayBuffer())
  const request = {
    url: () => origin,
    headers: () => Object.fromEntries(body.headers),
    postDataBuffer: () => bytes
  }
  await assert.rejects(
    mutationMatches(request, { fileId: 87, name: null }, 3000006134),
    /stage=superform-json; contentType=application\/x-www-form-urlencoded; fields=\[\]; chunks=0/
  )
})

test("JSON file mutations require exact file and page identity", async () => {
  const request = {
    headers: () => ({ "content-type": "application/json" }),
    postDataBuffer: () =>
      Buffer.from(JSON.stringify({ siteId, pageId: 3000006135, fileId: 87 }))
  }
  assert.equal(
    await mutationMatches(request, { fileId: 87, name: null }, 3000006135),
    true
  )
  assert.equal(
    await mutationMatches(request, { fileId: 78, name: null }, 3000006135),
    false
  )
  assert.equal(
    await mutationMatches(request, { fileId: 87, name: null }, 3000006134),
    false
  )
  for (const identity of [
    { siteId: String(siteId), pageId: 3000006135, fileId: 87 },
    { siteId, pageId: "3000006135", fileId: 87 },
    { siteId, pageId: 3000006135, fileId: "87" }
  ]) {
    assert.equal(
      await mutationMatches(
        { ...request, postDataBuffer: () => Buffer.from(JSON.stringify(identity)) },
        { fileId: 87, name: null },
        3000006135
      ),
      false
    )
  }
})

test("rollback text/plain JSON authorizes only matching fixture and file", async () => {
  const request = {
    headers: () => ({ "content-type": "text/plain;charset=UTF-8" }),
    postDataBuffer: () =>
      Buffer.from(
        JSON.stringify({
          siteId,
          pageId: 3000006134,
          fileId: 87,
          revisionNumber: 1,
          lastRevisionId: 42
        })
      )
  }
  const permitted = { fileId: 87, name: null }
  assert.equal(await mutationMatches(request, permitted, 3000006134), true)
  assert.equal(
    await mutationMatches(request, { fileId: 78, name: null }, 3000006134),
    false
  )
  assert.equal(await mutationMatches(request, permitted, 3000006135), false)
})

test("browser guard permits only fixture writes and records blocked requests", async () => {
  /** @type {(route: import("@playwright/test").Route) => Promise<void>} */
  let intercept = async () => assert.fail("route not registered")
  const context = {
    /** @param {string} pattern @param {typeof intercept} handler */
    async route(pattern, handler) {
      assert.equal(pattern, "**/*")
      intercept = handler
    }
  }
  // Only route registration is used by the guard.
  const guard = await guardBrowserWrites(
    /** @type {import("@playwright/test").BrowserContext} */ (
      /** @type {unknown} */ (context)
    ),
    ["local-action-proof:source", "local-action-proof:destination"]
  )
  /**
   * @param {string} url @param {string} method @param {string}
   *   resourceType @param {object} [data]
   */
  async function dispatch(url, method, resourceType, data = {}) {
    /** @type {string[]} */
    const actions = []
    const request = {
      url: () => url,
      method: () => method,
      resourceType: () => resourceType,
      headers: () => ({ "content-type": "application/json" }),
      postDataBuffer: () => Buffer.from(JSON.stringify(data))
    }
    const route = {
      request: () => request,
      continue: async () => {
        actions.push("continue")
      },
      abort: async () => {
        actions.push("abort")
      }
    }
    await intercept(
      /** @type {import("@playwright/test").Route} */ (/** @type {unknown} */ (route))
    )
    return actions
  }
  assert.deepEqual(
    await dispatch("https://d3g0gp89917ko0.cloudfront.net/base.css", "GET", "stylesheet"),
    ["continue"]
  )
  assert.deepEqual(await dispatch("https://example.org/image", "GET", "image"), ["abort"])
  assert.deepEqual(await dispatch("https://example.org/write", "POST", "fetch"), [
    "abort"
  ])
  assert.deepEqual(await dispatch(`${origin}/-/login`, "POST", "fetch"), ["continue"])
  guard.allow("local-action-proof:source", "fileEdit", 87)
  const action = `${origin}/local-action-proof:source?/fileEdit`
  assert.deepEqual(
    await dispatch(action, "POST", "fetch", { siteId, pageId: 3000006135, fileId: 87 }),
    ["abort"]
  )
  assert.deepEqual(
    await dispatch(action, "POST", "fetch", { siteId, pageId: 3000006134, fileId: 87 }),
    ["continue"]
  )
  assert.deepEqual(
    await dispatch(action, "POST", "fetch", { siteId, pageId: 3000006134, fileId: 87 }),
    ["abort"]
  )
  assert.deepEqual(guard.writes, ["/local-action-proof:source?/fileEdit"])
  assert.equal(guard.externalReads.length, 1)
  assert.equal(guard.blocked.length, 3)
  assert.equal(guard.decoded.length, 2)
})

test("action completion rejects failed results even with HTTP 200", async () => {
  /** @param {"success" | "failure"} type @param {unknown} data */
  const response = (type, data) => ({
    status: () => 200,
    json: async () => ({ type, status: 200, data: stringify(data) })
  })
  await assertActionSuccess(response("success", { res: { file_id: 87 } }), "fileRollback")
  await assert.rejects(
    assertActionSuccess(
      response("failure", { message: "rollback failed" }),
      "fileRollback"
    ),
    /fileRollback action failed/
  )
  await assert.rejects(
    assertActionSuccess(response("success", { form: { valid: true } }), "fileRollback"),
    /fileRollback missing result/
  )
})
