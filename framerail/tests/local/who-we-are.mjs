import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

/** @param {string | Uint8Array} value */
const digest = (value) => createHash("sha256").update(value).digest("hex")

/**
 * @typedef {{ x: number; y: number; width: number; height: number }} Rect
 *
 * @typedef {{
 *   viewport: { width: number; height: number }
 *   page_title: string
 *   content_text_sha256: string
 *   content_rect: Rect
 *   images: { path: string; rect: Rect; sha256: string }[]
 * }} Baseline
 */

/** @param {Rect} actual @param {Rect} expected @param {string} label */
function compareRect(actual, expected, label) {
  for (const key of /** @type {const} */ (["x", "y", "width", "height"])) {
    assert.ok(
      Math.abs(actual[key] - expected[key]) < 1,
      `${label} ${key}: ${actual[key]} vs ${expected[key]}`
    )
  }
}

test("who-we-are matches source article, image bytes, and layout", async () => {
  const passwordFile = process.env.COBALT_LOCAL_PASSWORD_FILE
  const baselineFile = process.env.COBALT_WHO_BASELINE
  assert.ok(passwordFile && baselineFile, "protected local fixture paths required")
  /** @type {Baseline} */
  const baseline = JSON.parse(await readFile(baselineFile, "utf8"))
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      viewport: baseline.viewport,
      httpCredentials: {
        username: "cobalt",
        password: (await readFile(passwordFile, "utf8")).trim(),
        origin: "http://127.0.0.1:3090"
      }
    })
    const page = await context.newPage()
    /** @type {string[]} */
    const errors = []
    page.on("pageerror", (error) => errors.push(error.message))
    const response = await page.goto("http://127.0.0.1:3090/who-we-are", {
      waitUntil: "networkidle"
    })
    assert.ok(response)
    assert.equal(response.status(), 200)
    await expect(page.locator("#page-title")).toHaveText(baseline.page_title)
    await expect(page.locator("#side-bar")).toHaveCount(0)
    const content = page.locator("#page-content")
    assert.equal(
      digest((await content.innerText()).replace(/\s+/g, " ").trim()),
      baseline.content_text_sha256
    )
    await expect(content.locator("img")).toHaveCount(3)
    for (const image of baseline.images) {
      const element = content.locator(`img[src="${image.path}"]`)
      assert.equal(
        await element.evaluate(
          (node) =>
            node instanceof HTMLImageElement && node.complete && node.naturalWidth > 0
        ),
        true
      )
      const result = await context.request.get(`http://127.0.0.1:3090${image.path}`)
      assert.equal(result.status(), 200)
      assert.match(result.headers()["content-type"], /^image\//)
      assert.equal(digest(await result.body()), image.sha256)
      const box = await element.boundingBox()
      assert.ok(box)
      compareRect(box, image.rect, image.path)
    }
    const box = await content.boundingBox()
    assert.ok(box)
    compareRect(box, baseline.content_rect, "article")
    assert.deepEqual(errors, [])
  } finally {
    await browser.close()
  }
})
