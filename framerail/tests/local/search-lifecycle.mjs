import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { setTimeout as delay } from "node:timers/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const preview = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const meili = "http://127.0.0.1:27700"
const siteId = 6000000
const pollIntervalMs = 500
const pollTimeoutMs = 60_000

/**
 * @typedef {import("../../src/lib/server/deepwell/search").SearchHit} SearchHit
 *
 *
 * @typedef {import("../../src/lib/server/deepwell/search").SearchPage} SearchPage
 *
 *
 * @typedef {{
 *   sacrificial: true
 *   previewUrl: string
 *   backendUrl: string
 *   meiliUrl: string
 *   siteId: 6000000
 *   siteSlug: "cobalt-company"
 *   databaseLabel: "cobalt_local_full"
 *   username: "cobalt-import"
 *   slug: string
 *   title: string
 *   tags: string[]
 *   initialToken: string
 *   editedToken: string
 * }} Fixture
 */

/** @param {string} path */
async function readFixture(path) {
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(path, "utf8"))
  assert.equal(fixture.sacrificial, true, "explicit sacrificial fixture required")
  assert.equal(fixture.previewUrl, preview)
  assert.equal(fixture.backendUrl, backend)
  assert.equal(fixture.meiliUrl, meili)
  assert.equal(fixture.siteId, siteId)
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.username, "cobalt-import")
  assert.match(fixture.slug, /^local-search-proof:roundtrip-[a-z0-9]{12,}$/)
  assert.ok(fixture.title?.trim(), "fixture title required")
  assert.ok(Array.isArray(fixture.tags) && fixture.tags.length, "fixture tags required")
  assert.ok(fixture.tags.every((tag) => /^[a-z0-9-]+$/.test(tag)))
  assert.match(fixture.initialToken, /^[a-z][a-z0-9]{15,}$/)
  assert.match(fixture.editedToken, /^[a-z][a-z0-9]{15,}$/)
  assert.notEqual(fixture.initialToken, fixture.editedToken)
  return fixture
}

/**
 * @param {import("@playwright/test").APIRequestContext} client
 * @param {string | null} sessionToken
 * @param {string} pageRef
 * @param {string} method
 * @param {unknown} params
 */
async function rpc(client, sessionToken, pageRef, method, params) {
  /** @type {Record<string, string>} */
  const headers = {
    "X-Deepwell-Site-Id": String(siteId),
    "X-Deepwell-Page": pageRef
  }
  if (sessionToken) headers["X-Deepwell-Session-Token"] = sessionToken
  const response = await client.post(backend, {
    headers,
    data: { jsonrpc: "2.0", id: 1, method, params }
  })
  assert.equal(response.status(), 200, `${method} HTTP status`)
  const payload = await response.json()
  assert.ok(!payload.error, `${method} RPC error code ${payload.error?.code ?? "?"}`)
  return payload.result
}

/**
 * @param {import("@playwright/test").APIRequestContext} client @param
 *   {string | null} token @param {string} slug
 */
async function readPage(client, token, slug) {
  return rpc(client, token, slug, "page_get", { site_id: siteId, page: slug })
}

/**
 * @param {import("@playwright/test").APIRequestContext} client @param
 *   {string} token @param {string} slug @param {string} query
 */
async function search(client, token, slug, query) {
  /** @type {SearchPage} */
  const page = await rpc(client, token, slug, "page_search", {
    query,
    offset: 0,
    limit: 20
  })
  assert.ok(Array.isArray(page?.hits), "page_search must return hits")
  return page.hits
}

/**
 * @param {import("@playwright/test").APIRequestContext} client
 * @param {string} token
 * @param {string} slug
 * @param {string} query
 * @param {number} pageId
 * @param {boolean} present
 */
async function waitForSearch(client, token, slug, query, pageId, present) {
  const deadline = Date.now() + pollTimeoutMs
  while (true) {
    const hits = await search(client, token, slug, query)
    const matches = hits.filter((hit) => hit.page_id === pageId)
    assert.ok(matches.length <= 1, "search must not duplicate fixture page")
    if (present && matches.length === 1) return matches[0]
    if (!present && hits.length === 0) return undefined
    if (Date.now() >= deadline) {
      assert.fail(`page_search fixture ${present ? "appearance" : "removal"} timed out`)
    }
    await delay(pollIntervalMs)
  }
}

/**
 * @param {SearchHit | undefined} hit @param {Fixture} fixture @param
 *   {string} slug
 * @param {string} token @param {number} pageId
 */
function assertHit(hit, fixture, slug, token, pageId) {
  assert.ok(hit, "search hit required")
  assert.equal(hit.page_id, pageId, "search page ID")
  assert.equal(hit.title, fixture.title, "search title")
  assert.equal(hit.slug, slug, "search slug")
  assert.deepEqual(hit.tags, fixture.tags, "search tags")
  assert.ok(hit.snippet.includes(token), "search snippet must contain current body token")
}

/**
 * @param {import("@playwright/test").BrowserContext} context @param
 *   {Fixture} fixture @param {string} password
 */
async function login(context, fixture, password) {
  const page = await context.newPage()
  await page.goto(`${preview}/-/login`, { waitUntil: "networkidle" })
  await expect(page.locator("#login")).toBeVisible()
  await page.locator('#login [name="nameOrEmail"]').fill(fixture.username)
  await page.locator('#login [name="password"]').fill(password)
  await page.locator("#login button[type=submit]").click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (entry) => entry.name === "wikijump_token" && entry.domain === "127.0.0.1"
  )
  assert.ok(cookie, "login must establish browser session")
  return decodeURIComponent(cookie.value)
}

/**
 * @param {import("@playwright/test").APIRequestContext} client @param
 *   {string} token @param {Fixture} fixture
 */
async function exerciseLifecycle(client, token, fixture) {
  const movedSlug = `${fixture.slug}-moved`
  assert.equal(
    await readPage(client, token, fixture.slug),
    null,
    "source slug must be unused"
  )
  assert.equal(await readPage(client, token, movedSlug), null, "move slug must be unused")
  const session = await rpc(client, token, fixture.slug, "session_get", [token])
  assert.ok(Number.isSafeInteger(session?.user_id), "session must identify user")
  const actor = { site_id: siteId, user_id: session.user_id, ip_address: "127.0.0.1" }

  const created = await rpc(client, token, fixture.slug, "page_create", {
    ...actor,
    slug: fixture.slug,
    title: fixture.title,
    wikitext: fixture.initialToken,
    tags: fixture.tags,
    revision_comments: "Local search lifecycle create"
  })
  assert.equal(created.slug, fixture.slug)
  assert.ok(Number.isSafeInteger(created.page_id), "create page ID")
  assert.ok(Number.isSafeInteger(created.revision_id), "create revision ID")
  const pageId = created.page_id
  const initial = await waitForSearch(
    client,
    token,
    fixture.slug,
    fixture.initialToken,
    pageId,
    true
  )
  assertHit(initial, fixture, fixture.slug, fixture.initialToken, pageId)

  const edited = await rpc(client, token, fixture.slug, "page_edit", {
    ...actor,
    page: pageId,
    last_revision_id: created.revision_id,
    wikitext: fixture.editedToken,
    revision_comments: "Local search lifecycle edit"
  })
  assert.ok(Number.isSafeInteger(edited?.revision_id), "edit must create revision")
  await waitForSearch(client, token, fixture.slug, fixture.initialToken, pageId, false)
  const current = await waitForSearch(
    client,
    token,
    fixture.slug,
    fixture.editedToken,
    pageId,
    true
  )
  assertHit(current, fixture, fixture.slug, fixture.editedToken, pageId)

  const moved = await rpc(client, token, fixture.slug, "page_move", {
    ...actor,
    page: pageId,
    last_revision_id: edited.revision_id,
    new_slug: movedSlug,
    revision_comments: "Local search lifecycle move"
  })
  assert.equal(moved.old_slug, fixture.slug)
  assert.equal(moved.new_slug, movedSlug)
  assert.ok(Number.isSafeInteger(moved.revision_id), "move revision ID")
  const movedHit = await waitForSearch(
    client,
    token,
    movedSlug,
    fixture.editedToken,
    pageId,
    true
  )
  assertHit(movedHit, fixture, movedSlug, fixture.editedToken, pageId)

  await rpc(client, token, movedSlug, "page_delete", {
    ...actor,
    page: pageId,
    last_revision_id: moved.revision_id,
    revision_comments: "Local search lifecycle delete"
  })
  await waitForSearch(client, token, movedSlug, fixture.editedToken, pageId, false)

  const restored = await rpc(client, token, movedSlug, "page_restore", {
    ...actor,
    page_id: pageId,
    revision_comments: "Local search lifecycle restore"
  })
  assert.equal(restored.slug, movedSlug)
  assert.ok(Number.isSafeInteger(restored.revision_id), "restore revision ID")
  const restoredHit = await waitForSearch(
    client,
    token,
    movedSlug,
    fixture.editedToken,
    pageId,
    true
  )
  assertHit(restoredHit, fixture, movedSlug, fixture.editedToken, pageId)
  const page = await readPage(client, token, movedSlug)
  assert.equal(page?.page_id, pageId)
  assert.equal(page.slug, movedSlug)
  assert.equal(page.revision_id, restored.revision_id)
}

test("local authenticated page lifecycle updates real search after each mutation", async () => {
  const fixturePath = process.env.COBALT_SEARCH_LIFECYCLE_FIXTURE
  const previewPasswordPath = process.env.COBALT_LOCAL_PASSWORD_FILE
  const adminPasswordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(
    fixturePath && previewPasswordPath && adminPasswordPath,
    "explicit fixture and two local password file paths required"
  )
  const fixture = await readFixture(fixturePath)
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      httpCredentials: {
        username: "cobalt",
        password: (await readFile(previewPasswordPath, "utf8")).trim(),
        origin: preview
      }
    })
    try {
      const password = (await readFile(adminPasswordPath, "utf8")).trim()
      const token = await login(context, fixture, password)
      await exerciseLifecycle(context.request, token, fixture)
    } finally {
      await context.close()
    }
  } finally {
    await browser.close()
  }
})
