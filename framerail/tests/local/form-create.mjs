import assert from "node:assert/strict"
import { randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

// Creates one sacrificial character page on a local replica.
// COBALT_FORM_PREVIEW          gateway origin, e.g. http://127.0.0.1:3091
// COBALT_FORM_BACKEND          Deepwell JSON-RPC URL behind it
// COBALT_POC_HTPASSWD          gateway htpasswd (user name is read from it)
// COBALT_POC_PASSWORD_FILE     gateway password
// COBALT_LOCAL_ADMIN_PASSWORD_FILE  password of the local "cobalt-import" account
const preview = process.env.COBALT_FORM_PREVIEW
const backend = process.env.COBALT_FORM_BACKEND
const env = (name) => {
  const value = process.env[name]
  assert.ok(value, `${name} required`)
  return value
}

/** @param {string} slug */
async function readSource(slug) {
  const response = await fetch(backend, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "page_get",
      params: { site_id: 6000000, page: slug, details: { wikitext: true } }
    })
  })
  return (await response.json()).result
}

test("NewPage creates a character through the category form", async () => {
  assert.ok(preview && backend, "preview origin and backend required")
  const username = (await readFile(env("COBALT_POC_HTPASSWD"), "utf8"))
    .split(":")[0]
    .trim()
  const password = (await readFile(env("COBALT_POC_PASSWORD_FILE"), "utf8")).trim()
  const admin = (await readFile(env("COBALT_LOCAL_ADMIN_PASSWORD_FILE"), "utf8")).trim()
  const suffix = randomBytes(4).toString("hex")
  const title = `Form Proof ${suffix}`
  const slug = `character:form-proof-${suffix}`

  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      httpCredentials: { username, password, origin: preview }
    })
    const page = await context.newPage()
    await page.goto(`${preview}/-/login`, { waitUntil: "networkidle" })
    await page.locator('#login [name="nameOrEmail"]').fill("cobalt-import")
    await page.locator('#login [name="password"]').fill(admin)
    await page.locator("#login button[type=submit]").click()
    await expect(page.locator("#login")).toHaveCount(0)

    await page.goto(`${preview}/new-profile`, { waitUntil: "networkidle" })
    const newPage = page.locator(
      '.new-page-box form:has(input[value="Create Character Profile"])'
    )
    await newPage.locator('[name="pageName"]').fill(title)
    await newPage.locator('[type="submit"]').click()
    await page.waitForURL(new RegExp(`/${slug}/edit/true/title/`), {
      waitUntil: "networkidle"
    })

    const editor = page.locator("#editor")
    await expect(editor.locator('[name="title"]')).toHaveValue(title)
    await expect(editor.locator('textarea[name="wikitext"]')).toHaveCount(0)
    await editor.getByLabel("Name", { exact: true }).fill(`Proof ${suffix} Tester`)
    await editor.getByLabel("Female", { exact: true }).check()
    const saved = page.waitForResponse(
      (response) =>
        response.request().method() === "POST" && response.url().includes("?/edit")
    )
    await editor.locator('[type="submit"]').click()
    assert.equal((await saved).status(), 200, "create must succeed")
    await page.waitForURL(new RegExp(`/${slug}$`))

    const stored = await readSource(slug)
    assert.equal(stored.title, title)
    const lines = stored.wikitext.split("\n")
    assert.deepEqual(lines.slice(0, 5), [
      "player: ''",
      `name: 'Proof ${suffix} Tester'`,
      "pronunciation: '@@'",
      "aka: '@@'",
      "portrait: '@@'"
    ])
    assert.ok(lines.includes("sex: female"), stored.wikitext)
    await expect(page.locator("#page-content")).toContainText(`Proof ${suffix} Tester`)
  } finally {
    await browser.close()
  }
})
