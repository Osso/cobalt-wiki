import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

// Read-only: logs in, clicks Edit and checks the editor opens in view; never saves.
// COBALT_EDIT_URL              existing page, e.g. https://cobalt-company.sakuin.org/character:arric
// COBALT_POC_HTPASSWD          gateway htpasswd (user name is read from it)
// COBALT_POC_PASSWORD_FILE     gateway password
// COBALT_EDITOR_USER           account allowed to edit the page
// COBALT_EDITOR_PASSWORD_FILE  that account's password
const url = process.env.COBALT_EDIT_URL
const htpasswd = process.env.COBALT_POC_HTPASSWD
const passwordFile = process.env.COBALT_POC_PASSWORD_FILE
const editor = process.env.COBALT_EDITOR_USER
const editorPasswordFile = process.env.COBALT_EDITOR_PASSWORD_FILE

test("Edit opens the page editor and scrolls to it", async () => {
  assert.ok(
    url && htpasswd && passwordFile && editor && editorPasswordFile,
    "page URL, gateway files and editor account required"
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
    const origin = new URL(url).origin
    await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
    await page.locator('#login [name="nameOrEmail"]').fill(editor)
    await page
      .locator('#login [name="password"]')
      .fill((await readFile(editorPasswordFile, "utf8")).trim())
    await page.locator("#login button[type=submit]").click()
    await expect(page.locator("#login")).toHaveCount(0)

    await page.goto(url, { waitUntil: "networkidle" })
    await page.locator("#edit-button").click()
    await page.waitForURL(`${url}/edit`)
    const actionArea = page.locator("#action-area")
    await expect(actionArea.locator("input").first()).toBeVisible()
    await expect(actionArea).toBeInViewport()
  } finally {
    await browser.close()
  }
})
