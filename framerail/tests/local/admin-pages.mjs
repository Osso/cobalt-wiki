import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { after, before, test } from "node:test"
import { chromium, expect } from "@playwright/test"

const origin = "http://127.0.0.1:3090"
/** @type {import("@playwright/test").Browser} */
let browser

before(async () => {
  browser = await chromium.launch({ executablePath: "/usr/bin/chromium", headless: true })
})
after(async () => {
  await browser?.close()
})

test("Applications loads its imported dashboard and framework assets", async () => {
  const context = await browser.newContext()
  try {
    const page = await context.newPage()
    const errors = []
    page.on("pageerror", (error) => errors.push(error.message))
    const response = await page.goto(`${origin}/_applications`, {
      waitUntil: "networkidle"
    })
    assert.equal(response?.status(), 200)
    await expect(
      page.getByRole("heading", { name: "New Applications", exact: true })
    ).toBeVisible()
    await expect(
      page.getByRole("heading", { name: "Accepted Applications", exact: true })
    ).toBeVisible()
    assert.deepEqual(errors, [])
  } finally {
    await context.close()
  }
})

test("legacy Site Manager opens administration for an admin and denies anonymous access", async () => {
  const passwordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(passwordPath, "explicit local admin password file required")
  const context = await browser.newContext()
  try {
    const page = await context.newPage()
    await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
    await page.locator('#login [name="nameOrEmail"]').fill("cobalt-import")
    await page
      .locator('#login [name="password"]')
      .fill((await readFile(passwordPath, "utf8")).trim())
    await page.locator('#login button[type="submit"]').click()
    await expect(page.locator("#login")).toHaveCount(0)
    const response = await page.goto(`${origin}/_admin`, { waitUntil: "networkidle" })
    assert.equal(response?.status(), 200)
    await expect(
      page.getByRole("heading", { name: "Site Manager", exact: true })
    ).toBeVisible()
    await expect(page.getByRole("button", { name: "Edit", exact: true })).toBeVisible()
    await expect(
      page.getByRole("link", { name: "Site members", exact: true }).last()
    ).toBeVisible()
    await page.getByRole("button", { name: "Edit", exact: true }).click()
    await expect(page.locator('#editor input[name="name"]')).toHaveValue("Cobalt Company")
    await page.getByRole("button", { name: "Cancel", exact: true }).click()
  } finally {
    await context.close()
  }
  const anonymous = await browser.newContext()
  try {
    const page = await anonymous.newPage()
    const response = await page.goto(`${origin}/_admin`, { waitUntil: "networkidle" })
    assert.equal(response?.status(), 401)
    await expect(page.locator("#editor")).toHaveCount(0)
  } finally {
    await anonymous.close()
  }
})

test("Digest Writings renders complete multi-row tables rather than literal delimiters", async () => {
  const context = await browser.newContext()
  try {
    const page = await context.newPage()
    const response = await page.goto(`${origin}/digest-writings`, {
      waitUntil: "networkidle"
    })
    assert.equal(response?.status(), 200)
    const content = page.locator("#page-content")
    await expect(content).toBeVisible()
    assert.ok(
      !(await content.innerText()).includes("||"),
      "digest rows must render as tables"
    )
    const tables = content.locator("table.wiki-content-table")
    assert.equal(await tables.count(), 2)
    for (const table of await tables.all()) {
      assert.ok(
        (await table.locator("tr").count()) > 2,
        "fixture requires multiple data rows"
      )
      await expect(table.locator("tr").first()).toContainText("Date Created")
      assert.ok((await table.locator('a[href^="/writing:"]').count()) > 1)
    }
  } finally {
    await context.close()
  }
})
