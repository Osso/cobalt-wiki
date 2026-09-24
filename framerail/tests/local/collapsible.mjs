import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

// Read-only: opens and closes a collapsible block on an existing page.
// COBALT_COLLAPSIBLE_URL     page with a Wikidot collapsible, e.g.
//                            https://cobalt-company.sakuin.org/writing:2026-09-22-light-and-darkness
// COBALT_POC_HTPASSWD        gateway htpasswd (user name is read from it)
// COBALT_POC_PASSWORD_FILE   gateway password
const url = process.env.COBALT_COLLAPSIBLE_URL
const htpasswd = process.env.COBALT_POC_HTPASSWD
const passwordFile = process.env.COBALT_POC_PASSWORD_FILE

test("collapsible block shows one label and swaps parts on click", async () => {
  assert.ok(url && htpasswd && passwordFile, "page URL and gateway files required")
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
    const block = page.locator(".collapsible-block").first()
    const folded = block.locator(":scope > .collapsible-block-folded")
    const unfolded = block.locator(":scope > .collapsible-block-unfolded")
    const content = unfolded.locator(".collapsible-block-content")

    await expect(folded).toBeVisible()
    await expect(unfolded).toBeHidden()

    await folded.locator("a").click()
    await expect(folded).toBeHidden()
    await expect(content).toBeVisible()
    assert.equal(page.url(), new URL(url).href, "toggle must not navigate")

    await unfolded.locator(".collapsible-block-unfolded-link a").first().click()
    await expect(folded).toBeVisible()
    await expect(unfolded).toBeHidden()
  } finally {
    await browser.close()
  }
})
