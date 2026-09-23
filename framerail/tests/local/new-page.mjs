import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

// Read-only: every case stops before the editor saves anything.
// COBALT_NEW_PAGE_URL        replica origin, e.g. https://cobalt-company.sakuin.org
// COBALT_POC_HTPASSWD        gateway htpasswd (user name is read from it)
// COBALT_POC_PASSWORD_FILE   gateway password
const origin = process.env.COBALT_NEW_PAGE_URL
const htpasswd = process.env.COBALT_POC_HTPASSWD
const passwordFile = process.env.COBALT_POC_PASSWORD_FILE

/**
 * @param {import("@playwright/test").Page} page @param {string} button
 *   @param {string} name
 */
async function submit(page, button, name) {
  await page.goto(`${origin}/new-writing`, { waitUntil: "networkidle" })
  const form = page.locator(`.new-page-box form:has(input[value="${button}"])`)
  await form.locator('[name="pageName"]').fill(name)
  await form.locator('[type="submit"]').click()
}

/** @param {import("@playwright/test").Page} page @param {string} message */
async function expectError(page, message) {
  await expect(page.locator("#owindow-1.error .content")).toContainText(message)
  assert.equal(new URL(page.url()).pathname, "/new-writing")
}

test("NewPage forms validate names and open the new page's editor", async () => {
  assert.ok(
    origin && htpasswd && passwordFile,
    "replica origin and gateway files required"
  )
  const username = (await readFile(htpasswd, "utf8")).split(":")[0].trim()
  const password = (await readFile(passwordFile, "utf8")).trim()
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({ httpCredentials: { username, password } })
    const page = await context.newPage()

    await submit(page, "Create Writing", "")
    await expectError(page, "You should provide a page name.")

    await submit(page, "Create Writing", "Bad Title")
    await expectError(page, "The page name is not in the required format.")

    await submit(page, "Create Arc", "Badlands")
    await expectError(page, "The page arc:badlands already exists.")

    await submit(page, "Create Writing", "(2099-01-01) New Page Probe")
    await page.waitForURL(/\/writing:2099-01-01-new-page-probe\//)
    assert.equal(
      new URL(page.url()).pathname,
      "/writing:2099-01-01-new-page-probe/edit/true/title/(2099-01-01)%20New%20Page%20Probe"
    )
  } finally {
    await browser.close()
  }
})
