import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const preview = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"

/** @param {unknown} value */
const digest = (value) => createHash("sha256").update(JSON.stringify(value)).digest("hex")

/**
 * @typedef {{
 *   sacrificial: true
 *   siteSlug: "cobalt-company"
 *   databaseLabel: "cobalt_local_full"
 *   siteId: 6000000
 *   username: "cobalt-import"
 *   slug: string
 *   title: string
 *   wikitext: string
 *   tags: string[]
 * }} Fixture
 */

/** @param {string} path */
async function readFixture(path) {
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(path, "utf8"))
  assert.equal(fixture.sacrificial, true, "sacrificial fixture required")
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.siteId, 6000000)
  assert.equal(fixture.username, "cobalt-import")
  assert.match(fixture.slug, /^local-create-proof:roundtrip-[a-z0-9-]{8,}$/)
  assert.ok(fixture.title?.trim(), "fixture title required")
  assert.ok(fixture.wikitext?.trim(), "fixture wikitext required")
  assert.ok(Array.isArray(fixture.tags) && fixture.tags.length, "fixture tags required")
  assert.ok(
    fixture.tags.every((tag) => /^[a-z0-9-]+$/.test(tag)),
    "simple tags required"
  )
  return fixture
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string | null} sessionToken
 * @param {string} [slug]
 */
async function readPage(request, fixture, sessionToken, slug = fixture.slug) {
  const response = await request.post(backend, {
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "page_view",
      params: {
        site_id: fixture.siteId,
        locales: ["en"],
        session_token: sessionToken,
        route: { slug, extra: "" }
      }
    }
  })
  assert.equal(response.status(), 200, "local backend page_view must respond")
  const payload = await response.json()
  assert.ok(!payload.error, `page_view failed (code ${payload.error?.code ?? "?"})`)
  return payload.result
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {Fixture} fixture
 */
async function assertMissing(request, fixture) {
  const view = await readPage(request, fixture, null)
  assert.equal(view?.type, "missing", "sacrificial slug must remain uncreated")
}

/**
 * @param {import("@playwright/test").Browser} browser @param {string}
 *   previewPassword
 */
async function newPreviewContext(browser, previewPassword) {
  return browser.newContext({
    httpCredentials: { username: "cobalt", password: previewPassword, origin: preview }
  })
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
  assert.ok(cookie, "login must create a real browser session")
  return { page, sessionToken: decodeURIComponent(cookie.value) }
}

/**
 * @param {import("@playwright/test").BrowserContext} context @param
 *   {Fixture} fixture
 */
async function assertAnonymousCreateDenied(context, fixture) {
  const page = await context.newPage()
  await page.goto(`${preview}/${fixture.slug}`, { waitUntil: "networkidle" })
  await expect(page.locator("#editor")).toHaveCount(0)
  await page.goto(`${preview}/${fixture.slug}/edit`, { waitUntil: "networkidle" })
  await expect(page.locator("#editor")).toHaveCount(0)
  const response = await context.request.post(`${preview}/${fixture.slug}?/edit`, {
    headers: { accept: "application/json", "x-sveltekit-action": "true" },
    form: {
      pageId: "0",
      siteId: String(fixture.siteId),
      lastRevisionId: "0",
      title: fixture.title,
      altTitle: "",
      tags: fixture.tags.join(" "),
      comments: "",
      wikitext: fixture.wikitext
    }
  })
  const action = await response.json()
  assert.equal(action.type, "failure", "anonymous create action must reject writes")
  assert.ok(action.status >= 400, "anonymous create action must fail")
  await assertMissing(context.request, fixture)
}

/** @param {import("@playwright/test").Page} page @param {Fixture} fixture */
async function createViaEditor(page, fixture) {
  await page.goto(`${preview}/${fixture.slug}`, { waitUntil: "networkidle" })
  await page.locator(`a[href="/${fixture.slug}/edit"]`).click()
  const editor = page.locator("#editor")
  await expect(editor).toBeVisible()
  await editor.locator('[name="title"]').fill(fixture.title)
  await editor.locator('[name="wikitext"]').fill(fixture.wikitext)
  await editor.locator('[name="tags"]').fill(fixture.tags.join(" "))
  const savedResponse = page.waitForResponse(
    (response) =>
      response.request().method() === "POST" && response.url().includes("?/edit")
  )
  await editor.locator('[type="submit"]').click()
  assert.equal((await savedResponse).status(), 200, "authenticated create must succeed")
  await expect(editor).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} sessionToken
 */
async function assertSaved(request, fixture, sessionToken) {
  const view = await readPage(request, fixture, sessionToken)
  assert.equal(view?.type, "found", "created page must be found")
  assert.equal(
    digest(view.data.page_revision.title),
    digest(fixture.title),
    "saved title digest"
  )
  assert.equal(digest(view.data.wikitext), digest(fixture.wikitext), "saved body digest")
  assert.equal(
    digest(view.data.page_revision.tags),
    digest(fixture.tags),
    "saved tags digest"
  )
}

test("authenticated missing-page create persists title, body and tags after reload", async () => {
  const fixturePath = process.env.COBALT_CREATE_PAGE_FIXTURE
  const previewPasswordPath = process.env.COBALT_LOCAL_PASSWORD_FILE
  const adminPasswordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(
    fixturePath && previewPasswordPath && adminPasswordPath,
    "explicit sacrificial fixture, preview password and admin password file paths required"
  )
  const fixture = await readFixture(fixturePath)
  const previewPassword = (await readFile(previewPasswordPath, "utf8")).trim()
  const adminPassword = (await readFile(adminPasswordPath, "utf8")).trim()
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const anonymous = await newPreviewContext(browser, previewPassword)
    try {
      const template = await readPage(
        anonymous.request,
        fixture,
        null,
        "local-create-proof:_template"
      )
      assert.equal(template?.type, "missing", "plain-text category must have no template")
      await assertMissing(anonymous.request, fixture)
      await assertAnonymousCreateDenied(anonymous, fixture)
    } finally {
      await anonymous.close()
    }

    const context = await newPreviewContext(browser, previewPassword)
    try {
      const { page, sessionToken } = await login(context, fixture, adminPassword)
      await createViaEditor(page, fixture)
      await assertSaved(context.request, fixture, sessionToken)
      await page.reload({ waitUntil: "networkidle" })
      await expect(page.locator("#editor")).toHaveCount(0)
      await assertSaved(context.request, fixture, sessionToken)
    } finally {
      await context.close()
    }
  } finally {
    await browser.close()
  }
})
