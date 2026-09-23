import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const sprite = `${origin}/cobalt-editor/icons1.png`
const toolbarName = "Wikitext formatting"
const sourceSelector = '#editor [name="wikitext"]'

/**
 * @typedef {{
 *   sacrificial: true
 *   siteId: 6000000
 *   siteSlug: "cobalt-company"
 *   databaseLabel: "cobalt_local_full"
 *   username: "cobalt-import"
 *   existingSlug: string
 *   missingSlug: string
 * }} Fixture
 */

/** @param {string} path */
async function readFixture(path) {
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(path, "utf8"))
  assert.equal(fixture.sacrificial, true, "sacrificial fixture required")
  assert.equal(fixture.siteId, 6000000)
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.username, "cobalt-import")
  assert.match(fixture.existingSlug, /^local-create-proof:roundtrip-[a-z0-9-]{8,}$/)
  assert.match(fixture.missingSlug, /^local-preview-proof:[a-z0-9-]{8,}$/)
  assert.notEqual(fixture.existingSlug, fixture.missingSlug)
  return fixture
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} slug
 * @param {string} token
 */
async function readPage(request, fixture, slug, token) {
  const response = await request.post(backend, {
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "page_view",
      params: {
        site_id: fixture.siteId,
        locales: ["en"],
        session_token: token,
        route: { slug, extra: "" }
      }
    }
  })
  assert.equal(response.status(), 200, `${slug} page_view status`)
  const payload = await response.json()
  assert.ok(!payload.error, `${slug} page_view error code ${payload.error?.code ?? "?"}`)
  return payload.result
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {string[]} denied
 */
async function denyWritesAndExternalRequests(context, denied) {
  await context.route("**/*", async (route) => {
    const request = route.request()
    const url = new URL(request.url())
    const local = url.origin === origin
    const action = url.search.startsWith("?/") ? url.search.slice(2) : null
    const allowedPost =
      request.method() === "POST" &&
      (url.pathname === "/-/login" ||
        (url.pathname.endsWith("/edit") &&
          action !== null &&
          ["preview", "editorPages", "editorAttachments", "draftGet"].includes(action)))
    if (local && (["GET", "HEAD"].includes(request.method()) || allowedPost)) {
      await route.continue()
      return
    }
    // Imported theme styles attempt read-only CDN fetches; block them without
    // confusing a prevented stylesheet fetch with a forbidden write.
    if (request.method() !== "GET" || request.resourceType() !== "stylesheet") {
      denied.push(`${request.method()} ${url.origin}${url.pathname}`)
    }
    await route.abort("blockedbyclient")
  })
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {Fixture} fixture
 * @param {string} password
 */
async function login(context, fixture, password) {
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  await page.locator('#login [name="nameOrEmail"]').fill(fixture.username)
  await page.locator('#login [name="password"]').fill(password)
  await page.locator('#login button[type="submit"]').click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (entry) => entry.name === "wikijump_token" && entry.domain === "127.0.0.1"
  )
  assert.ok(cookie, "authenticated browser session required")
  return { page, token: decodeURIComponent(cookie.value) }
}

/** @param {import("@playwright/test").Page} page @param {string} slug */
async function openEditor(page, slug) {
  await page.goto(`${origin}/${slug}/edit`, { waitUntil: "networkidle" })
  await expect(page.locator(sourceSelector)).toBeVisible()
  // Existing drafts are never restored or modified by this acceptance script.
  const savedDraft = page.getByRole("dialog", { name: "Saved draft" })
  if (await savedDraft.isVisible()) {
    await savedDraft.getByRole("button", { name: "Edit Original" }).click()
  }
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} value
 * @param {string} selected
 */
async function setSelection(page, value, selected = "tail") {
  const input = page.locator(sourceSelector)
  await input.fill(value)
  const start = value.indexOf(selected)
  assert.ok(start >= 0, "selection text must exist")
  await input.evaluate(
    (element, range) => {
      if (!(element instanceof HTMLTextAreaElement)) throw new Error("textarea required")
      element.focus()
      element.setSelectionRange(range.start, range.end)
    },
    { start, end: start + selected.length }
  )
  return { input, start, end: start + selected.length }
}

/** @param {import("@playwright/test").Page} page */
async function selection(page) {
  return page.locator(sourceSelector).evaluate((element) => {
    if (!(element instanceof HTMLTextAreaElement)) throw new Error("textarea required")
    return {
      focused: document.activeElement === element,
      start: element.selectionStart,
      end: element.selectionEnd,
      value: element.value
    }
  })
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} button
 * @param {string} name
 */
async function openWizard(page, button, name) {
  await page
    .getByRole("toolbar", { name: toolbarName })
    .getByRole("button", { name: button, exact: true })
    .click()
  const dialog = page.getByRole("dialog", { name })
  await expect(dialog).toBeVisible()
  return dialog
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} button
 * @param {string} name
 * @param {boolean} escape
 */
async function assertCancelRestoresSelection(page, button, name, escape = false) {
  const before = await selection(page)
  const dialog = await openWizard(page, button, name)
  if (escape) await page.keyboard.press("Escape")
  else await dialog.getByRole("button", { name: "Cancel" }).click()
  await expect(dialog).toHaveCount(0)
  assert.deepEqual(
    await selection(page),
    before,
    `${name} cancellation preserves source and range`
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").Locator} dialog
 */
async function insert(page, dialog) {
  await dialog.getByRole("button", { name: "Insert code" }).click()
  await expect(dialog).toHaveCount(0)
  assert.equal(
    (await selection(page)).focused,
    true,
    "editor focus restored after insertion"
  )
  return page.locator(sourceSelector).inputValue()
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} slug
 * @param {string} expectedSource
 */
async function preview(page, slug, expectedSource) {
  const responsePromise = page.waitForResponse(
    (response) =>
      response.request().method() === "POST" &&
      response.url().includes(`/${slug}/edit?/preview`)
  )
  await page.locator("#edit-preview-button").click()
  const response = await responsePromise
  assert.equal(response.status(), 200, "draft preview must succeed")
  const posted = response.request()
  const body = posted.postDataBuffer()
  assert.ok(body, "preview form body required")
  const request = new Request(posted.url(), {
    method: "POST",
    headers: posted.headers(),
    body: new Uint8Array(body).buffer
  })
  const payload = (await request.formData()).get("payload")
  assert.ok(typeof payload === "string", "preview payload required")
  assert.equal(JSON.parse(payload).wikitext, expectedSource, "preview uses editor source")
  const region = page.locator('section[aria-label="Page preview"]')
  await expect(region).toHaveAttribute("aria-busy", "false")
  return region
}

/** @param {import("@playwright/test").Page} page @param {string} slug */
async function testTable(page, slug) {
  await openEditor(page, slug)
  const original = "+ Wizard proof\n\ntail"
  await setSelection(page, original)
  await assertCancelRestoresSelection(page, "table wizard", "Table wizard")
  const dialog = await openWizard(page, "table wizard", "Table wizard")
  await dialog.getByLabel("Number of rows:").fill("2")
  await dialog.getByLabel("Number of columns:").fill("2")
  await dialog.getByLabel("First row as header").check()
  const source = await insert(page, dialog)
  assert.ok(source.includes("||~ header ||~ header ||"), "two source headers")
  assert.ok(source.includes("|| cell-content || cell-content ||"), "two source cells")
  assert.ok(source.endsWith("tail"), "selected text remains after table")
  const region = await preview(page, slug, source)
  await expect(region.locator("table th")).toHaveCount(2)
  await expect(region.locator("table td")).toHaveCount(2)
}

/** @param {import("@playwright/test").Page} page @param {string} slug */
async function testCode(page, slug) {
  await openEditor(page, slug)
  await setSelection(page, "+ Wizard proof\n\ntail")
  await assertCancelRestoresSelection(
    page,
    "code block wizard",
    "Code block wizard",
    true
  )
  const dialog = await openWizard(page, "code block wizard", "Code block wizard")
  await dialog.getByLabel("Code type:").selectOption("CSS")
  const source = await insert(page, dialog)
  assert.match(source, /\[\[code type="CSS"\]\]\ntail\n\[\[\/code\]\]/)
  const region = await preview(page, slug, source)
  await expect(region.locator("pre")).toContainText("tail")
}

/** @param {import("@playwright/test").Page} page @param {string} slug */
async function testUrl(page, slug) {
  await openEditor(page, slug)
  await setSelection(page, "+ Wizard proof\n\ntail")
  await assertCancelRestoresSelection(page, "URL link wizard", "URL link wizard")
  const dialog = await openWizard(page, "URL link wizard", "URL link wizard")
  await dialog.getByLabel("URL:").fill(`${origin}/cobalt-editor/icons1.png`)
  await dialog.getByLabel("Anchor text:").fill("Local sprite")
  await dialog.getByLabel("Open in a new window").check()
  const source = await insert(page, dialog)
  assert.ok(source.includes(`[*${sprite} Local sprite]tail`), "selected text preserved")
  const region = await preview(page, slug, source)
  const link = region.getByRole("link", { name: "Local sprite" })
  await expect(link).toHaveAttribute("href", sprite)
  await expect(link).toHaveAttribute("target", "_blank")
}

/** @param {import("@playwright/test").Page} page @param {string} slug */
async function testPageLink(page, slug) {
  await openEditor(page, slug)
  await setSelection(page, "+ Wizard proof\n\ntail")
  await assertCancelRestoresSelection(page, "page link wizard", "Page link wizard")
  const dialog = await openWizard(page, "page link wizard", "Page link wizard")
  await dialog.getByLabel("Page name:").fill("local")
  const matches = dialog.getByRole("list", { name: "Matching pages" }).getByRole("button")
  await expect.poll(() => matches.count()).toBeGreaterThanOrEqual(2)
  const suggestions = (await matches.allTextContents()).map((text) => text.split(" (")[0])
  assert.ok(
    suggestions.every((slug) => slug.startsWith("local")),
    "slug-prefix suggestions only"
  )
  assert.deepEqual(suggestions, [...suggestions].sort(), "slug-sorted suggestions")
  const matchText = await matches.first().textContent()
  assert.ok(matchText, "lookup result must contain a page name")
  const destination = matchText.split(" (")[0]
  assert.ok(destination.length >= 2, "real authorized lookup returns a slug")
  await matches.first().click()
  await dialog.getByLabel("Anchor text (optional):").fill("Open matched page")
  const source = await insert(page, dialog)
  assert.ok(source.includes(`[[[${destination} |Open matched page]]]tail`))
  const region = await preview(page, slug, source)
  await expect(region.getByRole("link", { name: "Open matched page" })).toHaveAttribute(
    "href",
    new RegExp(`/${destination.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(?:/|$)`)
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} slug
 * @param {boolean} hasAttachments
 */
async function testImage(page, slug, hasAttachments) {
  await openEditor(page, slug)
  await setSelection(page, "+ Wizard proof\n\ntail")
  await assertCancelRestoresSelection(page, "image wizard", "Image wizard")
  const dialog = await openWizard(page, "image wizard", "Image wizard")
  if (hasAttachments) {
    await dialog.getByRole("radio", { name: "attached file" }).check()
    await expect(dialog.getByText("No attached files available.")).toBeVisible()
    await dialog.getByRole("radio", { name: "external image (via URL)" }).check()
  } else {
    await expect(dialog.getByRole("radio", { name: "attached file" })).toHaveCount(0)
  }
  await dialog.getByLabel("Image URL:").fill(sprite)
  await dialog.getByLabel("Position:").selectOption("c")
  const source = await insert(page, dialog)
  assert.ok(
    source.includes(`[[=image ${sprite}]]tail`),
    "position and selection preserved"
  )
  const region = await preview(page, slug, source)
  const image = region.locator(`img[src*="cobalt-editor/icons1.png"]`)
  await expect(image).toBeVisible()
  assert.equal(
    await image.evaluate((element) => {
      if (!(element instanceof HTMLImageElement)) throw new Error("image required")
      return element.complete && element.naturalWidth > 0
    }),
    true
  )
}

/** @param {import("@playwright/test").Page} page @param {string} slug */
async function testEquation(page, slug) {
  await openEditor(page, slug)
  const original = "+ Wizard proof\n\n[[math Eq1]]\nx+y\n[[/math]]\ntail"
  await setSelection(page, original)
  await assertCancelRestoresSelection(
    page,
    "equation reference",
    "Equation reference wizard",
    true
  )
  const dialog = await openWizard(page, "equation reference", "Equation reference wizard")
  await expect(dialog.getByLabel("Please select equation label:")).toHaveValue("Eq1")
  await expect(dialog.getByText("x+y", { exact: true })).toBeVisible()
  const source = await insert(page, dialog)
  assert.ok(source.includes("Eq.([[eref Eq1]])tail"), "label and selected text preserved")
  const region = await preview(page, slug, source)
  await expect(region).toContainText("x+y")
  await expect(region).not.toContainText("[[eref Eq1]]")
  await expect(region.locator(".wj-equation-ref-marker")).toHaveText("Eq1")
  await expect(region).toContainText("Eq.(Eq1)")
}

test("six editor wizards insert and preview without saving local pages", async () => {
  const fixturePath = process.env.COBALT_PAGE_PREVIEW_FIXTURE
  const basicPasswordPath = process.env.COBALT_LOCAL_PASSWORD_FILE
  const accountPasswordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(
    fixturePath && basicPasswordPath && accountPasswordPath,
    "explicit preview fixture, Basic Auth and account password files required"
  )
  const fixture = await readFixture(fixturePath)
  const basicPassword = (await readFile(basicPasswordPath, "utf8")).trim()
  const accountPassword = (await readFile(accountPasswordPath, "utf8")).trim()
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      httpCredentials: { username: "cobalt", password: basicPassword, origin }
    })
    /** @type {string[]} */
    const denied = []
    try {
      await denyWritesAndExternalRequests(context, denied)
      const { page, token } = await login(context, fixture, accountPassword)
      const before = await readPage(context.request, fixture, fixture.existingSlug, token)
      const missingBefore = await readPage(
        context.request,
        fixture,
        fixture.missingSlug,
        token
      )
      assert.equal(before?.type, "found", "existing source page required")
      assert.equal(
        missingBefore?.type,
        "missing",
        "missing fixture page must remain absent"
      )
      assert.ok(!before.data.form, "wizard fixture must use raw wikitext")
      await context.tracing.start({ screenshots: true, snapshots: true })
      try {
        await testTable(page, fixture.existingSlug)
        await testCode(page, fixture.existingSlug)
        await testUrl(page, fixture.existingSlug)
        await testPageLink(page, fixture.existingSlug)
        await testImage(page, fixture.existingSlug, true)
        await testEquation(page, fixture.existingSlug)
        await testImage(page, fixture.missingSlug, false)
      } finally {
        await context.tracing.stop({
          path: "/tmp/claude/cobalt-wizard-browser-trace.zip"
        })
        const after = await readPage(
          context.request,
          fixture,
          fixture.existingSlug,
          token
        )
        assert.equal(after?.type, "found", "existing fixture must remain present")
        assert.deepEqual(
          after.data.page_revision,
          before.data.page_revision,
          "revision unchanged"
        )
        assert.equal(after.data.wikitext, before.data.wikitext, "stored source unchanged")
        assert.deepEqual(
          await readPage(context.request, fixture, fixture.missingSlug, token),
          missingBefore,
          "missing target must remain absent"
        )
        assert.deepEqual(denied, [], "browser must not make write or external requests")
      }
    } finally {
      await context.close()
    }
  } finally {
    await browser.close()
  }
})

/** @param {import("@playwright/test").Page} page */
async function insertAttachedImage(page) {
  await openEditor(page, "images")
  await setSelection(page, "+ Attachment wizard proof\n\ntail")
  const dialog = await openWizard(page, "image wizard", "Image wizard")
  await dialog.getByRole("radio", { name: "attached file" }).check()
  const select = dialog.getByLabel("Attached file:")
  await expect(select).toBeVisible()
  const name = await select.locator("option").nth(1).getAttribute("value")
  assert.ok(name, "imported page must expose at least one authorized image")
  await select.selectOption(name)
  await expectImageLoaded(dialog.getByAltText("Selected attachment"))
  const source = await insert(page, dialog)
  assert.ok(source.includes(`[[image ${name}]]tail`))
  const region = await preview(page, "images", source)
  await expectImageLoaded(region.locator("img").first())
}

/** @param {import("@playwright/test").Locator} image */
async function expectImageLoaded(image) {
  await expect
    .poll(() =>
      image.evaluate((element) => {
        if (!(element instanceof HTMLImageElement)) throw new Error("image required")
        return element.complete && element.naturalWidth > 0
      })
    )
    .toBe(true)
}

/** @param {import("@playwright/test").Page} page */
async function insertFlickrSource(page) {
  const dialog = await openWizard(page, "image wizard", "Image wizard")
  await dialog.getByRole("radio", { name: "Flickr.com" }).check()
  await dialog.getByLabel("Flickr image:").fill("not-a-photo")
  await dialog.getByRole("button", { name: "Insert code" }).click()
  await expect(dialog.getByRole("alert")).toContainText("Invalid Flickr")
  await dialog.getByLabel("Flickr image:").fill("123")
  const source = await insert(page, dialog)
  assert.ok(source.includes("[[image flickr:123]]"))
}

test("attached image wizard selects and previews an authorized existing image without saving", async () => {
  const fixturePath = process.env.COBALT_PAGE_PREVIEW_FIXTURE
  const basicPath = process.env.COBALT_LOCAL_PASSWORD_FILE
  const accountPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(
    fixturePath && basicPath && accountPath,
    "explicit local fixture and password files required"
  )
  const fixture = await readFixture(fixturePath)
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      httpCredentials: {
        username: "cobalt",
        password: (await readFile(basicPath, "utf8")).trim(),
        origin
      }
    })
    try {
      /** @type {string[]} */
      const denied = []
      await denyWritesAndExternalRequests(context, denied)
      const { page, token } = await login(
        context,
        fixture,
        (await readFile(accountPath, "utf8")).trim()
      )
      const before = await readPage(context.request, fixture, "images", token)
      assert.equal(before.type, "found")
      try {
        await insertAttachedImage(page)
        await insertFlickrSource(page)
      } finally {
        const after = await readPage(context.request, fixture, "images", token)
        assert.equal(after.type, "found")
        assert.deepEqual(after.data.page_revision, before.data.page_revision)
        assert.equal(after.data.wikitext, before.data.wikitext)
        assert.deepEqual(denied, [])
      }
    } finally {
      await context.close()
    }
  } finally {
    await browser.close()
  }
})
