import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"
import { parse } from "../../node_modules/.pnpm/node_modules/devalue/index.js"

const preview = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const missingFormSlug = "writing:2000-01-01-watcher-controls-proof"

/**
 * @typedef {{
 *   sacrificial: true
 *   siteId: 6000000
 *   siteSlug: "cobalt-company"
 *   databaseLabel: "cobalt_local_full"
 *   username: "cobalt-import"
 *   existingSlug: string
 *   missingSlug: string
 *   formSlug: "local-acceptance:form-roundtrip"
 * }} Fixture
 */

/** @param {string} path */
async function readFixture(path) {
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(path, "utf8"))
  assert.equal(fixture.sacrificial, true, "sacrificial fixture required")
  assert.equal(fixture.siteId, 6000000)
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.username, "cobalt-import")
  assert.match(fixture.existingSlug, /^local-create-proof:roundtrip-[a-z0-9-]{8,}$/)
  assert.match(fixture.missingSlug, /^local-preview-proof:[a-z0-9-]{8,}$/)
  assert.equal(fixture.formSlug, "local-acceptance:form-roundtrip")
  assert.notEqual(fixture.missingSlug, missingFormSlug)
  return fixture
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} slug
 * @param {string} token
 */
async function readPage(request, fixture, slug, token) {
  const response = await request.post(backend, {
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "page_view",
      params: {
        site_id: fixture.siteId,
        locales: ["en"],
        session_token: token,
        route: { slug, extra: "" }
      }
    }
  })
  assert.equal(response.status(), 200, `page_view must respond for ${slug}`)
  const payload = await response.json()
  assert.ok(!payload.error, `page_view failed for ${slug}`)
  return payload.result
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {Fixture} fixture
 * @param {string} password
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
  assert.ok(cookie, "browser login must create a session")
  return { page, token: decodeURIComponent(cookie.value) }
}

/** @param {import("@playwright/test").Request} posted */
async function decodeSave(posted) {
  const buffer = posted.postDataBuffer()
  assert.ok(buffer, "save request must include a body")
  const request = new Request(posted.url(), {
    method: "POST",
    headers: posted.headers(),
    body: new Uint8Array(buffer)
  })
  const form = await request.formData()
  const encoded = form.getAll("__superform_json")
  assert.ok(encoded.length, "save must serialize Superforms JSON")
  return parse(encoded.join(""))
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} slug
 * @param {boolean} structured
 * @param {import("@playwright/test").Request[]} blockedWrites
 */
async function assertEditorSendsWatcherChoice(page, slug, structured, blockedWrites) {
  await page.goto(`${preview}/${slug}/edit`, { waitUntil: "networkidle" })
  await expect(page.locator("#editor")).toBeVisible()
  await expect(page.locator('#editor [name="wikitext"]')).toHaveCount(structured ? 0 : 1)
  const checkbox = page.locator("#editor").getByLabel("Do Not Notify Watchers", {
    exact: true
  })
  await expect(checkbox).toHaveCount(1)
  await expect(checkbox).not.toBeChecked()
  const submit = page.locator('#editor [type="submit"]')
  for (const suppress of [false, true]) {
    await expect(submit).toBeEnabled()
    if (suppress) await checkbox.check()
    const saveRequest = page.waitForRequest(
      (request) =>
        request.method() === "POST" && request.url().includes(`/${slug}/edit?/edit`)
    )
    await submit.click()
    const posted = await saveRequest
    const payload = await decodeSave(posted)
    assert.equal(
      payload.doNotNotifyWatchers,
      suppress,
      `${slug} save must serialize watcher choice ${suppress}`
    )
    await expect
      .poll(() => blockedWrites.at(-1))
      .toBe(posted, "save must be intercepted before server")
  }
}

test("four editor modes serialize default and opted-out watcher notifications without saving", async () => {
  const fixturePath = process.env.COBALT_PAGE_PREVIEW_FIXTURE
  const previewPasswordPath = process.env.COBALT_LOCAL_PASSWORD_FILE
  const adminPasswordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(
    fixturePath && previewPasswordPath && adminPasswordPath,
    "explicit sacrificial fixture, Basic Auth and account password files required"
  )
  const fixture = await readFixture(fixturePath)
  const previewPassword = (await readFile(previewPasswordPath, "utf8")).trim()
  const adminPassword = (await readFile(adminPasswordPath, "utf8")).trim()
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      httpCredentials: { username: "cobalt", password: previewPassword, origin: preview }
    })
    try {
      const { page, token } = await login(context, fixture, adminPassword)
      const modes = [
        { slug: fixture.existingSlug, structured: false, expected: "found" },
        { slug: fixture.missingSlug, structured: false, expected: "missing" },
        { slug: fixture.formSlug, structured: true, expected: "found" },
        { slug: missingFormSlug, structured: true, expected: "missing" }
      ]
      const before = await Promise.all(
        modes.map(async ({ slug, expected }) => {
          const view = await readPage(context.request, fixture, slug, token)
          assert.equal(view.type, expected, `${slug} fixture state`)
          return view
        })
      )
      /** @type {import("@playwright/test").Request[]} */
      const blockedWrites = []
      await context.route("**/*", async (route) => {
        const request = route.request()
        const url = new URL(request.url())
        if (request.method() !== "POST" || url.search === "?/draftGet") {
          await route.continue()
          return
        }
        blockedWrites.push(request)
        await route.abort()
      })
      for (const { slug, structured } of modes) {
        await assertEditorSendsWatcherChoice(page, slug, structured, blockedWrites)
      }
      assert.equal(blockedWrites.length, 8, "only eight intercepted saves allowed")
      const after = await Promise.all(
        modes.map(({ slug }) => readPage(context.request, fixture, slug, token))
      )
      assert.deepEqual(after, before, "existing and missing page state must not change")
    } finally {
      await context.close()
    }
  } finally {
    await browser.close()
  }
})
