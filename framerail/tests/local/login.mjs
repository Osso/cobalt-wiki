import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

// Signs in from a page's header and returns to it; changes no content.
// COBALT_LOGIN_URL             existing page, e.g. https://cobalt-company.sakuin.org/character:arric
// COBALT_POC_HTPASSWD          gateway htpasswd (user name is read from it)
// COBALT_POC_PASSWORD_FILE     gateway password
// COBALT_EDITOR_USER           account to sign in with
// COBALT_EDITOR_PASSWORD_FILE  that account's password
const url = process.env.COBALT_LOGIN_URL
const htpasswd = process.env.COBALT_POC_HTPASSWD
const passwordFile = process.env.COBALT_POC_PASSWORD_FILE
const account = process.env.COBALT_EDITOR_USER
const accountPasswordFile = process.env.COBALT_EDITOR_PASSWORD_FILE

test("header Sign in opens the site's login page and returns to the page", async () => {
  assert.ok(
    url && htpasswd && passwordFile && account && accountPasswordFile,
    "page URL, gateway files and account required"
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
    await page.goto(url, { waitUntil: "networkidle" })
    const status = page.locator("#login-status")
    await expect(status).toContainText("Create account or Sign in")

    await status.locator(".login-status-sign-in").click()
    await page.waitForURL(/\/-\/login\?origUrl=/)
    await expect(page.locator("#login-title")).toHaveText(/^Sign in to /)
    await expect(page.locator("#top-bar a").first()).toBeVisible()
    await expect(page.locator("#license-area")).not.toContainText("{$license}")

    await page.locator('#login [name="nameOrEmail"]').fill(account)
    await page
      .locator('#login [name="password"]')
      .fill((await readFile(accountPasswordFile, "utf8")).trim())
    await page.locator("#login button[type=submit]").click()
    await page.waitForURL(url)
    await expect(status.locator(".printuser")).not.toBeEmpty()
    await expect(status.locator("#my-account")).toHaveText("My account")
  } finally {
    await browser.close()
  }
})
