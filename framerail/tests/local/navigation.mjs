import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

test("public Cobalt navigation matches captured source geometry and has no sidebar", async () => {
  const passwordFile = process.env.COBALT_LOCAL_PASSWORD_FILE
  assert.ok(passwordFile, "local preview credential file required")
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      viewport: { width: 1440, height: 1000 },
      httpCredentials: {
        username: "cobalt",
        password: (await readFile(passwordFile, "utf8")).trim(),
        origin: "http://127.0.0.1:3090"
      }
    })
    const page = await context.newPage()
    await page.goto("http://127.0.0.1:3090/home:_public", { waitUntil: "networkidle" })
    const nav = page.locator("#top-bar")
    await expect(nav).toBeVisible()
    const box = await nav.boundingBox()
    assert.ok(box)
    assert.equal(box.height, 24, "captured source navigation height")
    assert.equal(box.y, 106, "captured source navigation position")
    await expect(page.locator("#side-bar")).toHaveCount(0)
    const structure = await nav.evaluate((element) => {
      const items = Array.from(element.querySelectorAll("li")).filter(
        (item) => !item.parentElement?.closest("li")
      )
      return {
        count: items.length,
        rows: items.map((item) => item.getBoundingClientRect().y),
        orphanLists: element.querySelectorAll("ul > ul").length
      }
    })
    assert.equal(structure.count, 7, "all source top-level navigation groups")
    assert.equal(new Set(structure.rows).size, 1, "navigation stays in one row")
    assert.equal(structure.orphanLists, 0)
    const home = nav.locator("li").first()
    const submenu = home.locator(":scope > ul")
    await expect(submenu).toHaveCSS("visibility", "hidden")
    await home.hover()
    await expect(submenu).toHaveCSS("visibility", "visible")
    assert.ok((await submenu.locator("a").count()) > 0)
  } finally {
    await browser.close()
  }
})
