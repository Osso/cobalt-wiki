import assert from "node:assert/strict"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"

/** @param {import("@playwright/test").APIRequestContext} request */
async function readBadges(request) {
  const response = await request.post(backend, {
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "page_view",
      params: {
        site_id: 6000000,
        locales: ["en"],
        session_token: null,
        route: { slug: "badges", extra: "" }
      }
    }
  })
  assert.equal(response.status(), 200)
  const payload = await response.json()
  assert.ok(!payload.error, "Badges page lookup must succeed")
  assert.equal(payload.result.type, "found")
  return {
    source: payload.result.data.wikitext,
    revisionId: payload.result.data.page_revision.revision_id
  }
}

test("Badges thumbnails open a bounded gallery viewer without navigating or saving", async () => {
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  const context = await browser.newContext()
  try {
    const before = await readBadges(context.request)
    const page = await context.newPage()
    /** @type {string[]} */
    const errors = []
    /** @type {string[]} */
    const writes = []
    page.on("pageerror", (error) => errors.push(error.message))
    page.on("request", (request) => {
      if (request.method() !== "GET" && request.method() !== "HEAD")
        writes.push(request.method())
    })
    await page.goto(`${origin}/badges`, { waitUntil: "networkidle" })
    const links = page.locator("#page-content .gallery-box .gallery-item a")
    const count = await links.count()
    assert.ok(count > 2, "real fixture must have multiple badges")
    const first = links.first()
    const firstUrl = await first.getAttribute("href")
    const secondUrl = await links.nth(1).getAttribute("href")
    assert.ok(firstUrl && secondUrl)
    await first.click()
    const viewer = page.getByRole("dialog", { name: "Gallery viewer" })
    await expect(viewer).toBeVisible()
    assert.equal(page.url(), `${origin}/badges`)
    await expect(viewer).toContainText(`Image 1 of ${count}`)
    const image = viewer.locator("img")
    await expect(image).toHaveAttribute("src", new URL(firstUrl, origin).href)
    await expect(image).toBeVisible()
    await expect
      .poll(() =>
        image.evaluate(
          (node) =>
            node instanceof HTMLImageElement && node.complete && node.naturalWidth > 0
        )
      )
      .toBe(true)
    await viewer.getByRole("button", { name: "Next", exact: true }).click()
    await expect(viewer).toContainText(`Image 2 of ${count}`)
    await expect(image).toHaveAttribute("src", new URL(secondUrl, origin).href)
    await viewer.getByRole("button", { name: "Previous", exact: true }).click()
    await expect(viewer).toContainText(`Image 1 of ${count}`)
    await viewer.getByRole("button", { name: "Close", exact: true }).click()
    await expect(viewer).toHaveCount(0)
    await expect(first).toBeFocused()
    await first.press("Enter")
    await expect(viewer).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(viewer).toHaveCount(0)
    await expect(first).toBeFocused()
    await links.last().click()
    await expect(viewer).toContainText(`Image ${count} of ${count}`)
    await expect(viewer.getByRole("button", { name: "Next", exact: true })).toBeDisabled()
    await viewer.getByRole("button", { name: "Close", exact: true }).click()
    assert.equal(page.url(), `${origin}/badges`)
    assert.deepEqual(errors, [])
    assert.deepEqual(writes, [])
    assert.deepEqual(await readBadges(context.request), before)
  } finally {
    await context.close()
    await browser.close()
  }
})
