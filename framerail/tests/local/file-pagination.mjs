import assert from "node:assert/strict"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"
const { stringify } = await import(
  new URL("../../node_modules/.pnpm/node_modules/devalue/index.js", import.meta.url).href
)

const origin = "http://127.0.0.1:3090"
const slug = "icons"
const files = Array.from({ length: 42 }, (_, index) => ({
  file_id: 100 + index,
  file_created_at: "2024-03-01T12:30:00Z",
  file_updated_at: null,
  file_deleted_at: index === 1 ? "2024-03-02T12:30:00Z" : null,
  page_id: 42,
  revision_id: 200 + index,
  revision_type: index === 1 ? "delete" : "create",
  revision_created_at: "2024-03-01T12:30:00Z",
  revision_number: 1,
  revision_user_id: 101,
  name: `file-${String(index + 1).padStart(2, "0")}.txt`,
  data: null,
  mime: "text/plain",
  size: 10,
  s3_hash: "abc",
  revision_comments: "",
  hidden_fields: []
}))

test("Files paginate alphabetically without URL navigation and clamp after refresh", async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: "/usr/bin/chromium"
  })
  try {
    const context = await browser.newContext()
    const page = await context.newPage()
    let listed = files
    await context.route("**/*", (route) => {
      const request = route.request()
      if (request.method() === "GET") return route.continue()
      if (
        request.method() === "POST" &&
        request.url() === `${origin}/${slug}?/fileList`
      ) {
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({
            type: "success",
            status: 200,
            data: stringify({ res: listed })
          })
        })
      }
      throw new Error(`Unexpected browser write: ${request.method()} ${request.url()}`)
    })
    const response = await page.goto(`${origin}/${slug}`, { waitUntil: "networkidle" })
    assert.equal(response?.status(), 200)
    const open = page
      .locator("#page-options-bottom #files-button, .other-actions .button-files")
      .first()
    await open.click()
    const pane = page.locator(".file-panel")
    await expect(pane).toBeVisible()
    const names = pane.locator(".file-row .name a")
    const pager = pane.getByRole("navigation", { name: "Files pages" })
    await expect(names).toHaveCount(20)
    assert.deepEqual(
      await names.allTextContents(),
      files.slice(0, 20).map((file) => file.name)
    )
    await expect(pane.locator(".file-row.is-deleted")).toHaveCount(1)
    await expect(pane).toContainText("410 Bytes")
    await pager.getByRole("button", { name: "Page 3" }).click()
    await expect(names).toHaveCount(2)
    assert.deepEqual(
      await names.allTextContents(),
      files.slice(40).map((file) => file.name)
    )
    assert.equal(page.url(), `${origin}/${slug}`)
    await pager.getByRole("button", { name: "Previous page" }).click()
    await expect(names).toHaveCount(20)
    assert.deepEqual(
      await names.allTextContents(),
      files.slice(20, 40).map((file) => file.name)
    )
    await pager.getByRole("button", { name: "Page 1" }).click()
    assert.deepEqual(
      await names.allTextContents(),
      files.slice(0, 20).map((file) => file.name)
    )
    await pager.getByRole("button", { name: "Next page" }).click()
    assert.deepEqual(
      await names.allTextContents(),
      files.slice(20, 40).map((file) => file.name)
    )
    await pager.getByRole("button", { name: "Next page" }).click()
    await expect(names).toHaveCount(2)
    listed = files.slice(0, 5)
    await pane.locator(".buttons input[value='Restore'], .deleted-file").first().click()
    await expect(names).toHaveCount(5)
    await expect(pager).toHaveCount(0)
    assert.deepEqual(
      await names.allTextContents(),
      listed.map((file) => file.name)
    )
    assert.equal(page.url(), `${origin}/${slug}`)
  } finally {
    await browser.close()
  }
})
