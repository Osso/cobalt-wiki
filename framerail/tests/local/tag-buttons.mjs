import assert from "node:assert/strict"
import { randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

// Writes one throwaway page, local-tag-proof:publish-<random>, on the
// isolated local stack (database cobalt_local_full, site 6000000).
// COBALT_LOCAL_ADMIN_PASSWORD_FILE  password of the cobalt-import user
// COBALT_LOCAL_PREVIEW              default http://127.0.0.1:3090
// COBALT_LOCAL_BACKEND              default http://127.0.0.1:2749/jsonrpc
const preview = process.env.COBALT_LOCAL_PREVIEW ?? "http://127.0.0.1:3090"
const backend = process.env.COBALT_LOCAL_BACKEND ?? "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000
const slug = `local-tag-proof:publish-${randomBytes(6).toString("hex")}`

// writing:_template's steps 2 and 5, without its instructions.
const wikitext = `[[iftags -_completed]]
[[button tags text="Open the Tags editor"]]
[[button set-tags +_completed -@@  text="Publish"]]
[[/iftags]]`
// "@@" is what an empty %%form_raw{...}%% field leaves; Publish removes it.
const draftTags = ["@@", "local-proof"]

const publish = 'a.wiki-standalone-button[data-button-type="set-tags"]'
const openTags = 'a.wiki-standalone-button[data-button-type="tags"]'

/** @param {import("@playwright/test").APIRequestContext} request */
async function readTags(request) {
  const response = await request.post(backend, {
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "page_view",
      params: {
        site_id: siteId,
        locales: ["en"],
        session_token: null,
        route: { slug, extra: "" }
      }
    }
  })
  const payload = await response.json()
  assert.equal(payload.result?.type, "found", "throwaway page must exist")
  return [...payload.result.data.page_revision.tags].sort()
}

/**
 * @param {import("@playwright/test").BrowserContext} context @param
 *   {string} password
 */
async function login(context, password) {
  const page = await context.newPage()
  await page.goto(`${preview}/-/login`, { waitUntil: "networkidle" })
  await page.locator('#login [name="nameOrEmail"]').fill("cobalt-import")
  await page.locator('#login [name="password"]').fill(password)
  await page.locator("#login button[type=submit]").click()
  await expect(page.locator("#login")).toHaveCount(0)
  return page
}

/**
 * Creates the draft through Framerail's own edit action, posted from the
 * page: the session cookie is Secure, which only the browser sends over
 * loopback HTTP.
 */
async function createDraft(/** @type {import("@playwright/test").Page} */ page) {
  await page.goto(`${preview}/${slug}`, { waitUntil: "networkidle" })
  const fields = {
    pageId: "0",
    siteId: String(siteId),
    lastRevisionId: "0",
    title: "Tag button proof",
    altTitle: "",
    tags: draftTags.join(" "),
    comments: "",
    wikitext
  }
  const result = await page.evaluate(async (fields) => {
    const response = await fetch("?/edit", {
      method: "POST",
      headers: { accept: "application/json", "x-sveltekit-action": "true" },
      body: new URLSearchParams(fields)
    })
    return response.json()
  }, fields)
  assert.equal(result.type, "success", `draft create failed: ${result.data}`)
}

test("Publish adds _completed, drops @@ and hides itself; anonymous Publish is refused", async () => {
  const passwordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(passwordPath, "admin password file path required")
  const password = (await readFile(passwordPath, "utf8")).trim()
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const admin = await browser.newContext()
    const page = await login(admin, password)
    await createDraft(page)

    const anonymous = await browser.newContext()
    try {
      const visitor = await anonymous.newPage()
      await visitor.goto(`${preview}/${slug}`, { waitUntil: "networkidle" })
      await visitor.locator(publish).click()
      await expect(visitor.locator("#modal-title")).toHaveText(
        "UNTRANSLATED:You don't have permission to edit this page"
      )
      assert.deepEqual(await readTags(anonymous.request), [...draftTags].sort())
    } finally {
      await anonymous.close()
    }

    await page.goto(`${preview}/${slug}`, { waitUntil: "networkidle" })
    // Keyboard: the rendered href makes the button focusable; Enter activates it.
    await expect(page.locator(openTags)).toHaveAttribute("href", /^javascript:;$/)
    await page.locator(openTags).focus()
    await page.keyboard.press("Enter")
    await expect(page.locator("#page-tags [name=tags]")).toHaveValue(draftTags.join(" "))

    const reloaded = page.waitForEvent("load")
    await page.locator(publish).click()
    await reloaded
    await expect(page.locator(publish)).toHaveCount(0)
    await expect(page.locator("#modal-title")).toHaveCount(0)
    assert.deepEqual(await readTags(admin.request), ["_completed", "local-proof"])
    await admin.close()
  } finally {
    await browser.close()
  }
})
