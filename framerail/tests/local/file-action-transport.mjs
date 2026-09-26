import assert from "node:assert/strict"
import { createHash } from "node:crypto"
const { parse } = await import(
  new URL("../../node_modules/.pnpm/node_modules/devalue/index.js", import.meta.url).href
)

export const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
export const siteId = 6000000

/** @typedef {import("../../src/lib/server/deepwell/pageFile").PageFile} PageFile */
/** @typedef {import("../../src/lib/server/deepwell/views").PageView} PageView */
/**
 * @typedef {{
 *   headers: () => { [key: string]: string }
 *   postDataBuffer: () => Buffer | null
 *   url?: () => string
 * }} MutationRequest
 */

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug @param {string} method @param
 *   {unknown} params
 */
async function rpc(request, token, slug, method, params) {
  const response = await request.post(backend, {
    headers: {
      "X-Deepwell-Site-Id": String(siteId),
      "X-Deepwell-Page": slug,
      "X-Deepwell-Session-Token": token
    },
    data: { jsonrpc: "2.0", id: 1, method, params }
  })
  assert.equal(response.status(), 200, `${method} HTTP status`)
  const payload = await response.json()
  assert.ok(!payload.error, `${method} RPC error ${payload.error?.code ?? "?"}`)
  return payload.result
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug
 * @returns {Promise<PageView>}
 */
async function readPage(request, token, slug) {
  /** @type {PageView} */
  return rpc(request, token, slug, "page_view", {
    site_id: siteId,
    locales: ["en"],
    session_token: token,
    route: { slug, extra: "" }
  })
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug @param {number} pageId
 */
async function assertLivePage(request, token, slug, pageId) {
  const view = await readPage(request, token, slug)
  assert.equal(view.type, "found", `${slug} must remain live`)
  if (view.type !== "found") assert.fail("fixture page missing")
  assert.equal(view.data.page.page_id, pageId)
  assert.equal(view.data.page.slug, slug)
  return view.data
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug @param {number} pageId @param
 *   {boolean} deleted
 * @returns {Promise<PageFile[]>}
 */
async function listFiles(request, token, slug, pageId, deleted) {
  /** @type {PageFile[]} */
  return rpc(request, token, slug, "page_get_files", {
    site_id: siteId,
    page_id: pageId,
    deleted
  })
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug @param {number} pageId @param
 *   {number} fileId
 */
async function readFileBytes(request, token, slug, pageId, fileId) {
  /** @type {PageFile} */
  const file = await rpc(request, token, slug, "file_get", {
    site_id: siteId,
    page_id: pageId,
    file: fileId,
    details: { data: true }
  })
  assert.equal(file.file_id, fileId)
  assert.equal(file.page_id, pageId)
  assert.ok(
    typeof file.data === "string" && /^[a-f0-9]*$/.test(file.data),
    "file_get must return hex bytes"
  )
  const bytes = Buffer.from(file.data, "hex")
  assert.equal(file.size, bytes.length)
  assert.equal(file.s3_hash, createHash("sha512").update(bytes).digest("hex"))
  return { file, bytes }
}

/** @param {MutationRequest} request */
async function decodeMutation(request) {
  let stage = "body"
  let contentType = "missing"
  /** @type {string[]} */
  let fields = []
  let chunkCount = null
  try {
    const bytes = request.postDataBuffer()
    assert.ok(bytes, "mutation body required")
    stage = "content-type"
    const headers = request.headers()
    const header = headers["content-type"]
    const supportedType = [
      "application/json",
      "text/plain",
      "multipart/form-data",
      "application/x-www-form-urlencoded"
    ].find((type) => header?.startsWith(type))
    contentType = supportedType ?? (header ? "other" : "missing")
    if (contentType === "application/json" || header === "text/plain;charset=UTF-8") {
      stage = "json"
      return { data: JSON.parse(bytes.toString()), file: null, files: [] }
    }
    const isMultipart = contentType === "multipart/form-data"
    assert.ok(isMultipart || contentType === "application/x-www-form-urlencoded")
    if (!request.url) assert.fail("form mutation URL required")
    const body = new Request(request.url(), {
      method: "POST",
      headers,
      body: new Uint8Array(bytes)
    })
    stage = isMultipart ? "multipart-form" : "urlencoded-form"
    const form = await body.formData()
    const knownFields = new Set([
      "__superform_json",
      "__superform_file_file",
      "__superform_id",
      "siteId",
      "pageId",
      "lastRevisionId",
      "fileId",
      "destinationPage",
      "name",
      "comments",
      "file"
    ])
    fields = [
      ...new Set(
        [...form.keys()].map((field) => (knownFields.has(field) ? field : "(other)"))
      )
    ]
    stage = "superform-json"
    const chunks = form.getAll("__superform_json")
    chunkCount = chunks.length
    assert.ok(chunkCount, "Superforms JSON required")
    stage = "devalue"
    const data = parse(chunks.join(""))
    const files = [...form.entries()].flatMap(([field, file]) =>
      file instanceof File ? [{ field, name: file.name, type: file.type }] : []
    )
    return { data, file: form.get("__superform_file_file"), files }
  } catch (error) {
    const errorType =
      error instanceof assert.AssertionError
        ? "AssertionError"
        : error instanceof SyntaxError
          ? "SyntaxError"
          : "Error"
    throw new Error(
      `mutation decode failed (${errorType}; stage=${stage}; contentType=${contentType}; fields=${JSON.stringify(fields)}; chunks=${chunkCount ?? "unknown"})`,
      { cause: error }
    )
  }
}

/**
 * @param {MutationRequest} request @param {{ fileId: number | null; name:
 *   string | null }} permitted @param {number} pageId
 * @param {object[] | null} [diagnostics]
 */
async function mutationMatches(request, permitted, pageId, diagnostics = null) {
  const { data, file, files } = await decodeMutation(request)
  diagnostics?.push({
    siteId: typeof data?.siteId === "number" ? data.siteId : typeof data?.siteId,
    pageId: typeof data?.pageId === "number" ? data.pageId : typeof data?.pageId,
    fileId: typeof data?.fileId === "number" ? data.fileId : typeof data?.fileId,
    name: typeof data?.name === "string" ? data.name : typeof data?.name,
    files
  })
  if (typeof data !== "object" || data === null) return false
  if (data.siteId !== siteId || data.pageId !== pageId) return false
  if (permitted.fileId !== null) return data.fileId === permitted.fileId
  return (
    data.name === permitted.name && file instanceof File && file.name === permitted.name
  )
}

/**
 * @param {import("@playwright/test").Request} request @param {string[]}
 *   slugs
 */
function requestLabel(request, slugs) {
  const url = new URL(request.url())
  const fixturePath = url.origin === origin && slugs.includes(url.pathname.slice(1))
  const path =
    fixturePath || (url.origin === origin && url.pathname === "/-/login")
      ? url.pathname
      : ""
  const actions = [
    "fileUpload",
    "fileEdit",
    "fileMove",
    "fileDelete",
    "fileRestore",
    "fileRollback"
  ]
  const action =
    fixturePath && url.search.startsWith("?/") && actions.includes(url.search.slice(2))
      ? url.search
      : ""
  return `${request.method()} ${url.origin}${path}${action}`
}

/**
 * @typedef {{
 *   slug: string
 *   action: string
 *   fileId: number | null
 *   name: string | null
 * }} PermittedWrite
 *
 *
 * @typedef {{
 *   permitted: PermittedWrite | null
 *   writes: string[]
 *   blocked: string[]
 *   externalReads: string[]
 *   decoded: object[]
 * }} WriteState
 */

/**
 * @param {import("@playwright/test").Route} route @param {string[]} slugs
 * @param {WriteState} state
 */
async function routeBrowserRequest(route, slugs, state) {
  const request = route.request()
  const url = new URL(request.url())
  const isBaseStylesheet =
    url.origin === "https://d3g0gp89917ko0.cloudfront.net" &&
    request.method() === "GET" &&
    request.resourceType() === "stylesheet"
  if (isBaseStylesheet) {
    await route.continue()
    return false
  }
  if (url.origin !== origin) {
    await abortForeignRequest(route, slugs, state, url)
    return false
  }
  if (["GET", "HEAD", "OPTIONS"].includes(request.method())) {
    await route.continue()
    return false
  }
  const path = `${url.pathname}${url.search}`
  const login = path === "/-/login" || path === "/-/login?/login"
  const read = slugs.some((slug) =>
    ["fileList", "fileHistory"].some((action) => path === `/${slug}?/${action}`)
  )
  const mutation =
    state.permitted && path === `/${state.permitted.slug}?/${state.permitted.action}`
  if (request.method() === "POST" && (login || read)) {
    await route.continue()
    return false
  }
  if (request.method() === "POST" && mutation && state.permitted) {
    return routePermittedMutation(route, slugs, state, url, path, state.permitted)
  }
  state.blocked.push(requestLabel(request, slugs))
  await route.abort()
  return false
}

/**
 * @param {import("@playwright/test").Route} route @param {string[]} slugs
 * @param {WriteState} state @param {URL} url
 */
async function abortForeignRequest(route, slugs, state, url) {
  const request = route.request()
  const label = `${request.method()} ${url.origin}`
  if (["GET", "HEAD", "OPTIONS"].includes(request.method())) {
    state.externalReads.push(label)
  } else {
    state.blocked.push(`foreign origin ${requestLabel(request, slugs)}`)
  }
  await route.abort()
}

/**
 * @param {import("@playwright/test").Route} route @param {string[]} slugs
 * @param {WriteState} state @param {URL} url @param {string} path
 * @param {PermittedWrite} permitted
 */
async function routePermittedMutation(route, slugs, state, url, path, permitted) {
  const pageId = slugs.indexOf(permitted.slug) === 0 ? 3000006134 : 3000006135
  try {
    assert.ok(
      await mutationMatches(route.request(), permitted, pageId, state.decoded),
      "wrong fixture identity"
    )
  } catch (error) {
    state.blocked.push(`POST ${url.pathname}?/${permitted.action} ${String(error)}`)
    await route.abort()
    return false
  }
  state.writes.push(path)
  await route.continue()
  return true
}

/**
 * @param {import("@playwright/test").BrowserContext} context @param
 *   {string[]} slugs
 */
async function guardBrowserWrites(context, slugs) {
  /** @type {WriteState} */
  const state = {
    permitted: null,
    writes: [],
    blocked: [],
    externalReads: [],
    decoded: []
  }
  await context.route("**/*", async (route) => {
    const consumed = await routeBrowserRequest(route, slugs, state)
    if (consumed) state.permitted = null
  })
  return {
    writes: state.writes,
    blocked: state.blocked,
    externalReads: state.externalReads,
    decoded: state.decoded,
    /**
     * @param {string} slug @param {string} action @param {number | null}
     *   fileId @param {string | null} name
     */
    allow(slug, action, fileId, name = null) {
      assert.ok(slugs.includes(slug))
      assert.equal(state.permitted, null, "previous write not consumed")
      state.permitted = { slug, action, fileId, name }
    },
    assertConsumed() {
      assert.equal(state.permitted, null, "UI did not send expected mutation")
      assert.deepEqual(state.blocked, [], "unexpected browser write blocked")
    }
  }
}

/**
 * @param {Pick<import("@playwright/test").Response, "status" | "json">} response
 * @param {string} action
 */
async function assertActionSuccess(response, action) {
  assert.equal(response.status(), 200, `${action} HTTP status`)
  const result = await response.json()
  assert.equal(result.type, "success", `${action} action failed: ${result.status}`)
  const data = parse(result.data)
  assert.ok(data?.res, `${action} missing result`)
}

export {
  rpc,
  assertLivePage,
  listFiles,
  readFileBytes,
  mutationMatches,
  requestLabel,
  guardBrowserWrites,
  assertActionSuccess
}
