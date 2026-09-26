import assert from "node:assert/strict"
import { randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"
import {
  origin,
  siteId,
  rpc,
  assertLivePage,
  listFiles,
  readFileBytes
} from "./file-action-transport.mjs"
import { openFiles } from "./file-action-lifecycle.mjs"

const fixturePath =
  "/home/osso/.local/share/cobalt-wiki/local-full/page-action-fixture.json"
const pageId = 3000006134

/** @typedef {import("../../src/lib/server/deepwell/pageFile").PageFile} PageFile */
/** @typedef {import("@playwright/test").APIRequestContext} APIRequestContext */
/** @typedef {import("@playwright/test").BrowserContext} BrowserContext */
/** @typedef {import("@playwright/test").Page} Page */
/** @typedef {import("@playwright/test").Request} BrowserRequest */
/** @typedef {{ name: string; bytes: Buffer }} Upload */
/**
 * @typedef {{
 *   sacrificial: true
 *   siteId: number
 *   siteSlug: string
 *   databaseLabel: string
 *   username: string
 *   sourceSlug: string
 *   destinationSlug: string
 *   pageId: number
 *   destinationPageId: number
 *   created: { slug: string; pageId: number }[]
 * }} Fixture
 */

/** @param {string} path */
async function readFixture(path) {
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(path, "utf8"))
  assert.equal(fixture.sacrificial, true)
  assert.equal(fixture.siteId, siteId)
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.username, "cobalt-import")
  const match = /^local-action-proof:source-([a-f0-9]{16})$/.exec(fixture.sourceSlug)
  assert.ok(match, "unique disposable fixture required")
  assert.equal(fixture.destinationSlug, `local-action-proof:destination-${match[1]}`)
  assert.equal(fixture.pageId, pageId)
  assert.equal(fixture.destinationPageId, 3000006135)
  assert.deepEqual(fixture.created, [
    { slug: fixture.sourceSlug, pageId },
    { slug: fixture.destinationSlug, pageId: fixture.destinationPageId }
  ])
  return fixture
}

/**
 * @param {BrowserContext} context @param {Fixture} fixture @param {string}
 *   password
 */
async function login(context, fixture, password) {
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  await page.locator('#login [name="nameOrEmail"]').fill(fixture.username)
  await page.locator('#login [name="password"]').fill(password)
  await page.locator('#login button[type="submit"]').click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (item) => item.name === "wikijump_token" && item.domain === "127.0.0.1"
  )
  assert.ok(cookie, "admin browser login required")
  return { page, token: decodeURIComponent(cookie.value) }
}

/**
 * @param {BrowserRequest} request @param {number} revisionId @param
 *   {Upload[]} uploads
 */
async function assertNativeUpload(request, revisionId, uploads) {
  assert.match(
    request.headers()["content-type"] ?? "",
    /^multipart\/form-data; boundary=/
  )
  const bytes = request.postDataBuffer()
  assert.ok(bytes, "upload body required")
  const form = await new Request(request.url(), {
    method: "POST",
    headers: request.headers(),
    body: new Uint8Array(bytes)
  }).formData()
  assert.deepEqual([...form.keys()].sort(), [
    "comments",
    "file",
    "lastRevisionId",
    "name",
    "pageId",
    "siteId"
  ])
  assert.equal(form.get("siteId"), String(siteId))
  assert.equal(form.get("pageId"), String(pageId))
  assert.equal(form.get("lastRevisionId"), String(revisionId))
  assert.equal(form.get("comments"), "Batch proof")
  const file = form.get("file")
  assert.ok(file instanceof File, "one native file part required")
  assert.equal(form.getAll("file").length, 1)
  const actual = Buffer.from(await file.arrayBuffer())
  const upload = uploads.find(
    (candidate) => candidate.name === file.name && candidate.bytes.equals(actual)
  )
  assert.ok(upload, "unexpected upload file or bytes")
  assert.equal(form.get("name"), upload.name)
  return upload
}

/**
 * @param {BrowserContext} context @param {Fixture} fixture @param {number}
 *   revisionId
 * @param {Upload[]} uploads
 */
async function guardWrites(context, fixture, revisionId, uploads) {
  /** @type {Upload[]} */
  const posted = []
  /** @type {string[]} */
  const blocked = []
  await context.route("**/*", async (route) => {
    const request = route.request()
    const url = new URL(request.url())
    const method = request.method()
    if (
      url.origin === "https://d3g0gp89917ko0.cloudfront.net" &&
      method === "GET" &&
      request.resourceType() === "stylesheet"
    )
      return route.continue()
    if (url.origin === origin && ["GET", "HEAD", "OPTIONS"].includes(method)) {
      return route.continue()
    }
    const path = `${url.pathname}${url.search}`
    if (
      url.origin === origin &&
      method === "POST" &&
      ["/-/login", "/-/login?/login", `/${fixture.sourceSlug}?/fileList`].includes(path)
    )
      return route.continue()
    if (
      url.origin === origin &&
      method === "POST" &&
      path === `/${fixture.sourceSlug}?/fileUpload` &&
      posted.length < uploads.length
    ) {
      try {
        const upload = await assertNativeUpload(request, revisionId, uploads)
        assert.equal(upload, uploads[posted.length], "upload order / duplicate attempt")
        posted.push(upload)
        return route.continue()
      } catch (error) {
        blocked.push(
          `invalid fixture upload: ${error instanceof Error ? error.name : "Error"}`
        )
      }
    } else {
      blocked.push(`blocked ${method} ${url.origin === origin ? path : "foreign origin"}`)
    }
    await route.abort()
  })
  return { posted, blocked }
}

/**
 * @param {APIRequestContext} request @param {string} token @param
 *   {Fixture} fixture
 * @param {Map<number, PageFile>} baseline @param {Set<string>} names
 */
async function cleanup(request, token, fixture, baseline, names) {
  const current = await listFiles(request, token, fixture.sourceSlug, pageId, false)
  const created = current.filter((file) => !baseline.has(file.file_id))
  assert.ok(
    created.every((file) => names.has(file.name)),
    "foreign new file: no cleanup"
  )
  if (created.length === 0) return
  const session = await rpc(request, token, fixture.sourceSlug, "session_get", [token])
  assert.ok(Number.isSafeInteger(session?.user_id), "admin actor required")
  for (const file of created) {
    await rpc(request, token, fixture.sourceSlug, "file_delete", {
      site_id: siteId,
      page_id: pageId,
      user_id: session.user_id,
      file: file.file_id,
      last_revision_id: file.revision_id,
      revision_comments: "Tombstone disposable batch proof file"
    })
  }
}

/**
 * @param {Page} page @param {APIRequestContext} request @param {string}
 *   token
 * @param {Fixture} fixture
 */
async function exercise(page, request, token, fixture) {
  const originalPage = await assertLivePage(request, token, fixture.sourceSlug, pageId)
  const active = await listFiles(request, token, fixture.sourceSlug, pageId, false)
  const deleted = await listFiles(request, token, fixture.sourceSlug, pageId, true)
  const baseline = new Map([...active, ...deleted].map((file) => [file.file_id, file]))
  const suffix = randomBytes(8).toString("hex")
  const name = `batch-${suffix}-a.txt`
  const thirdName = `batch-${suffix}-c.txt`
  const uploads = [
    { name, bytes: Buffer.from(`first ${suffix}`) },
    { name, bytes: Buffer.from(`collision ${suffix}`) },
    { name: thirdName, bytes: Buffer.from(`third ${suffix}`) }
  ]
  assert.ok(![...baseline.values()].some((file) => [name, thirdName].includes(file.name)))
  const guardState = await guardWrites(
    page.context(),
    fixture,
    originalPage.page_revision.revision_id,
    uploads
  )
  /** @type {unknown} */
  let failure
  try {
    const pane = await openFiles(page, fixture.sourceSlug)
    await pane.locator(".upload-file, .buttons input[value='Upload']").click()
    const form = pane.locator("#file-upload")
    const input = form.locator('[name="file"]')
    await expect(input).toHaveAttribute("multiple", "")
    await input.setInputFiles(
      uploads.map(({ name, bytes }) => ({
        name,
        mimeType: "text/plain",
        buffer: bytes
      }))
    )
    await form.locator('[name="comments"]').fill("Batch proof")
    await form.evaluate((element) => {
      /** @type {HTMLFormElement & { uploadStates?: string[] }} */
      const observed = element
      observed.uploadStates = []
      const capture = () => {
        for (const row of observed.querySelectorAll("[data-upload-index]")) {
          const status = row.textContent ?? ""
          if (/uploading/i.test(status)) observed.uploadStates?.push(status)
        }
      }
      new MutationObserver(capture).observe(observed, {
        childList: true,
        characterData: true,
        subtree: true
      })
    })
    await form.locator('[type="submit"]').click()
    const rows = form.locator("[data-upload-index]")
    await expect(rows).toHaveCount(3)
    for (const [index, upload] of uploads.entries()) {
      const row = rows.locator(`[data-upload-index="${index}"]`)
      await expect(row).toContainText(upload.name)
      await expect(row).toContainText(index === 1 ? /failed/i : /uploaded/i)
    }
    await expect(rows.nth(1)).toContainText(/already exists|duplicate|collision/i)
    const uploadingStates = await form.evaluate(
      (element) =>
        /** @type {HTMLFormElement & { uploadStates?: string[] }} */ (element)
          .uploadStates ?? []
    )
    assert.ok(uploadingStates.length > 0, "uploading status must be visible")
    await page.waitForTimeout(500)
    assert.deepEqual(guardState.posted, uploads, "exactly three ordered upload POSTs")
    assert.deepEqual(guardState.blocked, [], "unexpected write or foreign request")
    const files = await listFiles(request, token, fixture.sourceSlug, pageId, false)
    const created = files.filter((file) => !baseline.has(file.file_id))
    assert.equal(
      created.length,
      2,
      "both successful files retained; collision not overwritten"
    )
    for (const upload of [uploads[0], uploads[2]]) {
      const matches = created.filter((file) => file.name === upload.name)
      assert.equal(matches.length, 1)
      const result = await readFileBytes(
        request,
        token,
        fixture.sourceSlug,
        pageId,
        matches[0].file_id
      )
      assert.deepEqual(result.bytes, upload.bytes)
    }
  } catch (error) {
    failure = error
  }
  try {
    await cleanup(request, token, fixture, baseline, new Set([name, thirdName]))
    const remaining = await listFiles(request, token, fixture.sourceSlug, pageId, false)
    const tombstones = await listFiles(request, token, fixture.sourceSlug, pageId, true)
    for (const [files, label] of [
      [remaining, "active"],
      [tombstones, "deleted"]
    ]) {
      assert.deepEqual(
        files
          .filter((file) => baseline.has(file.file_id))
          .sort((a, b) => a.file_id - b.file_id),
        (label === "active" ? active : deleted).sort((a, b) => a.file_id - b.file_id),
        `${label} baseline files unchanged`
      )
    }
    const pageAfter = await assertLivePage(request, token, fixture.sourceSlug, pageId)
    assert.equal(
      pageAfter.page_revision.revision_id,
      originalPage.page_revision.revision_id
    )
    assert.equal(pageAfter.wikitext, originalPage.wikitext)
  } catch (error) {
    if (failure)
      throw new AggregateError([failure, error], "batch proof and cleanup failed")
    throw error
  }
  if (failure) throw failure
}

test(
  "batch upload preserves successes across filename collision",
  { timeout: 120_000 },
  async () => {
    assert.equal(
      process.env.COBALT_PAGE_ACTION_FIXTURE,
      fixturePath,
      "explicit sacrificial fixture opt-in required"
    )
    const adminPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
    const gatewayPath = process.env.COBALT_LOCAL_PASSWORD_FILE
    assert.ok(adminPath && gatewayPath, "admin and gateway password file paths required")
    const fixture = await readFixture(fixturePath)
    const browser = await chromium.launch({
      executablePath: "/usr/bin/chromium",
      headless: true
    })
    try {
      const context = await browser.newContext({
        httpCredentials: {
          username: "cobalt",
          password: (await readFile(gatewayPath, "utf8")).trim(),
          origin
        }
      })
      try {
        const { page, token } = await login(
          context,
          fixture,
          (await readFile(adminPath, "utf8")).trim()
        )
        await exercise(page, context.request, token, fixture)
      } finally {
        await context.close()
      }
    } finally {
      await browser.close()
    }
  }
)
