import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {{ siteId: number; existingSlug: string }} fixture
 * @param {string} token
 */
async function readPageFingerprint(request, fixture, token) {
  const response = await request.post(backend, {
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "page_view",
      params: {
        site_id: fixture.siteId,
        locales: ["en"],
        session_token: token,
        route: { slug: fixture.existingSlug, extra: "" }
      }
    }
  })
  assert.equal(response.status(), 200, "read-only page_view HTTP status")
  const payload = await response.json()
  assert.ok(!payload.error, `read-only page_view RPC error ${payload.error?.code}`)
  assert.equal(payload.result?.type, "found", "fixture page must still exist")
  const page = payload.result.data
  return {
    source: createHash("sha256").update(page.wikitext).digest("hex"),
    revision: page.page_revision.revision_id
  }
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {{ existingSlug: string }} fixture
 */
async function guardBrowserWrites(context, fixture) {
  /** @type {string[]} */
  const intercepted = []
  /** @type {string[]} */
  const unexpected = []
  await context.route("**/*", (route) => {
    const request = route.request()
    if (["GET", "HEAD", "OPTIONS"].includes(request.method())) {
      return route.continue()
    }
    const url = new URL(request.url())
    if (
      request.method() === "POST" &&
      url.origin === origin &&
      url.pathname === "/-/login" &&
      ["", "?/login"].includes(url.search)
    ) {
      return route.continue()
    }
    if (
      request.method() !== "POST" ||
      url.origin !== origin ||
      url.pathname !== `/${fixture.existingSlug}` ||
      url.search !== "?/delete"
    ) {
      unexpected.push(`${request.method()} ${url.origin}${url.pathname}${url.search}`)
      return route.abort()
    }
    intercepted.push(`${url.origin}${url.pathname}${url.search}`)
    return route.fulfill({
      status: 403,
      contentType: "application/json",
      body: JSON.stringify({
        type: "failure",
        status: 403,
        data: { message: "Delete intercepted by local browser test" }
      })
    })
  })
  return { intercepted, unexpected }
}

/** @param {import("@playwright/test").Page} page */
async function openDeletePane(page) {
  await page.locator("#more-options-button").click()
  await page.locator("#delete-button").click()
  await expect(page.locator("#page-delete")).toBeVisible()
}

test("Wikidot direct Delete confirms before POST; Move remains immediate", async () => {
  const fixturePath = process.env.COBALT_PAGE_PREVIEW_FIXTURE
  const adminPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(fixturePath && adminPath, "protected local fixture paths required")
  const fixture = JSON.parse(await readFile(fixturePath, "utf8"))
  assert.equal(fixture.sacrificial, true)
  assert.equal(fixture.siteId, 6000000)
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.username, "cobalt-import")
  assert.match(fixture.existingSlug, /^local-create-proof:roundtrip-[a-z0-9-]{8,}$/)

  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const gatewayPath = process.env.COBALT_LOCAL_PASSWORD_FILE
    const gatewayPassword = gatewayPath
      ? (await readFile(gatewayPath, "utf8")).trim()
      : null
    const context = await browser.newContext({
      ...(gatewayPassword && {
        httpCredentials: { username: "cobalt", password: gatewayPassword, origin }
      })
    })
    try {
      const { intercepted, unexpected } = await guardBrowserWrites(context, fixture)
      const page = await context.newPage()
      await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
      await page.locator('#login [name="nameOrEmail"]').fill(fixture.username)
      await page
        .locator('#login [name="password"]')
        .fill((await readFile(adminPath, "utf8")).trim())
      await page.locator("#login button[type=submit]").click()
      await expect(page.locator("#login")).toHaveCount(0)
      const cookie = (await context.cookies()).find(
        (entry) => entry.name === "wikijump_token" && entry.domain === "127.0.0.1"
      )
      assert.ok(cookie, "browser login must create a session")
      const token = decodeURIComponent(cookie.value)
      const before = await readPageFingerprint(context.request, fixture, token)

      await page.goto(`${origin}/${fixture.existingSlug}`, { waitUntil: "networkidle" })
      await openDeletePane(page)
      await page.locator("#page-delete-option-delete").check()
      const confirmation = page.getByRole("dialog", { name: "Delete page?" })
      await page.locator('#page-delete input[type="submit"]').click()
      await expect(confirmation).toBeVisible()
      await expect(confirmation).toContainText(
        "Are you sure you want to completely wipe out this page?"
      )
      assert.deepEqual(intercepted, [], "Delete must wait for confirmation")
      await confirmation.getByRole("button", { name: "Cancel" }).click()
      await expect(confirmation).not.toBeVisible()
      assert.deepEqual(intercepted, [], "cancelled Delete must not POST")

      await page.locator('#page-delete input[type="submit"]').click()
      await expect(confirmation).toBeVisible()
      await page.keyboard.press("Escape")
      await expect(confirmation).not.toBeVisible()
      assert.deepEqual(intercepted, [], "Escape must not POST")

      await page.locator('#page-delete input[type="submit"]').click()
      await expect(confirmation).toBeVisible()
      await confirmation.getByRole("button", { name: "Delete page" }).click()
      await expect(confirmation).not.toBeVisible()
      await expect.poll(() => intercepted.length).toBe(1)
      assert.deepEqual(intercepted, [`${origin}/${fixture.existingSlug}?/delete`])

      await page.goto(`${origin}/${fixture.existingSlug}`, { waitUntil: "networkidle" })
      await openDeletePane(page)
      await expect(page.locator("#page-delete-option-move")).toBeChecked()
      await page.locator('#page-delete input[type="submit"]').click()
      await expect.poll(() => intercepted.length).toBe(2)
      await expect(
        page.locator('dialog[aria-labelledby="delete-confirmation-title"]')
      ).not.toBeVisible()
      assert.deepEqual(intercepted, [
        `${origin}/${fixture.existingSlug}?/delete`,
        `${origin}/${fixture.existingSlug}?/delete`
      ])
      const after = await readPageFingerprint(context.request, fixture, token)
      assert.deepEqual(after, before, "source and revision must remain unchanged")
      assert.deepEqual(unexpected, [], "all other browser writes must be blocked")
    } finally {
      await context.close()
    }
  } finally {
    await browser.close()
  }
})
