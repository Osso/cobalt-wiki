import assert from "node:assert/strict"
import { test } from "node:test"
import {
  assertActionSuccess,
  mutationMatches,
  origin,
  siteId
} from "./file-action-transport.mjs"
const { stringify } = await import(
  new URL("../../node_modules/.pnpm/node_modules/devalue/index.js", import.meta.url).href
)

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
