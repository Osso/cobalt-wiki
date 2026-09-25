import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const preview = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"

/**
 * @typedef {import("../../src/lib/server/deepwell/views").PageView} PageView
 *
 *
 * @typedef {Extract<PageView, { type: "found" }>["data"]} StoredPage
 *
 * @typedef {{
 *   sacrificial: true
 *   siteId: 6000000
 *   siteSlug: "cobalt-company"
 *   databaseLabel: "cobalt_local_full"
 *   username: "cobalt-import"
 *   existingSlug: string
 *   missingSlug: string
 *   formSlug: "local-acceptance:form-roundtrip"
 *   rawMarker: string
 *   formFieldName: string
 *   formFieldLabel: string
 *   formMarker: string
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
  assert.equal(fixture.formSlug, "local-acceptance:form-roundtrip")
  for (const key of /** @type {const} */ ([
    "rawMarker",
    "formFieldName",
    "formFieldLabel",
    "formMarker"
  ])) {
    assert.ok(fixture[key]?.trim(), `${key} required`)
  }
  assert.notEqual(fixture.rawMarker, fixture.formMarker)
  return fixture
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} slug
 * @param {string | null} sessionToken
 */
async function readPage(request, fixture, slug, sessionToken) {
  const response = await request.post(backend, {
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "page_view",
      params: {
        site_id: fixture.siteId,
        locales: ["en"],
        session_token: sessionToken,
        route: { slug, extra: "" }
      }
    }
  })
  assert.equal(response.status(), 200, `page_view must respond for ${slug}`)
  const payload = await response.json()
  assert.ok(!payload.error, `page_view failed (code ${payload.error?.code ?? "?"})`)
  /** @type {PageView} */
  const view = payload.result
  return view
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} slug
 * @param {string} token
 * @param {StoredPage} before
 */
async function assertStoredUnchanged(request, fixture, slug, token, before) {
  const after = await readPage(request, fixture, slug, token)
  assert.equal(after?.type, "found", `${slug} must remain present`)
  if (after.type !== "found") assert.fail(`${slug} must remain present`)
  assert.deepEqual(after.data.page_revision, before.page_revision, `${slug} revision`)
  assert.deepEqual(after.data.wikitext, before.wikitext, `${slug} source`)
  assert.deepEqual(after.data.form?.values, before.form?.values, `${slug} form values`)
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {Fixture} fixture
 * @param {string} password
 */
async function login(context, fixture, password) {
  const page = await context.newPage()
  await page.goto(`${preview}/-/login`, { waitUntil: "networkidle" })
  await expect(page.locator("#login")).toBeVisible()
  await page.locator('#login [name="nameOrEmail"]').fill(fixture.username)
  await page.locator('#login [name="password"]').fill(password)
  await page.locator("#login button[type=submit]").click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (entry) => entry.name === "wikijump_token" && entry.domain === "127.0.0.1"
  )
  assert.ok(cookie, "real browser login must create a session")
  return { page, token: decodeURIComponent(cookie.value) }
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} slug
 */
async function openEditor(page, slug) {
  await page.goto(`${preview}/${slug}/edit`, { waitUntil: "networkidle" })
  await expect(page.locator("#editor")).toBeVisible()
  await expect(page.locator("#edit-preview-button")).toBeVisible()
  await expect(page.locator("#editor #edit-preview-button")).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} slug
 */
async function clickPreview(page, slug) {
  const responsePromise = page.waitForResponse(
    (response) =>
      response.request().method() === "POST" &&
      response.url().includes(`/${slug}/edit?/preview`)
  )
  await page.locator("#edit-preview-button").click()
  const response = await responsePromise
  assert.equal(response.status(), 200, `${slug} preview action must succeed`)
  const region = page.locator('section[aria-label="Page preview"]')
  await expect(region).toHaveAttribute("aria-busy", "false")
  const posted = response.request()
  const postedBody = posted.postDataBuffer()
  assert.ok(postedBody, "preview request must contain a multipart body")
  const body = new Uint8Array(postedBody).buffer
  const request = new Request(posted.url(), {
    method: "POST",
    headers: posted.headers(),
    body
  })
  const form = await request.formData()
  const payload = form.get("payload")
  assert.ok(typeof payload === "string", "preview form payload required")
  return { region, submitted: JSON.parse(payload) }
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} marker
 */
async function fillFormattedSource(page, marker) {
  const input = page.locator('#editor [name="wikitext"]')
  const unformatted = `+ ${marker}\n{{${marker}}}\n${marker}`
  await input.fill(unformatted)
  await input.evaluate((element, length) => {
    if (!(element instanceof HTMLTextAreaElement)) {
      throw new Error("editor source must be a textarea")
    }
    element.focus()
    element.setSelectionRange(element.value.length - length, element.value.length)
  }, marker.length)
  await page
    .getByRole("toolbar", { name: "Wikitext formatting" })
    .getByRole("button", { name: "bold", exact: true })
    .click()
  const formatted = `+ ${marker}\n{{${marker}}}\n**${marker}**`
  await expect(input).toHaveValue(formatted)
  assert.deepEqual(
    await input.evaluate((element) => {
      if (!(element instanceof HTMLTextAreaElement)) {
        throw new Error("editor source must be a textarea")
      }
      return {
        focused: document.activeElement === element,
        start: element.selectionStart,
        end: element.selectionEnd
      }
    }),
    { focused: true, start: formatted.length, end: formatted.length }
  )
}

/**
 * @param {import("@playwright/test").Locator} region
 * @param {string} marker
 */
async function assertPreviewMonospaceSize(region, marker) {
  const code = region.locator("p code").filter({ hasText: marker })
  await expect(code).toHaveCount(1)
  const sizes = await code.evaluate((element) => {
    const paragraph = element.closest("p")
    if (!paragraph) throw new Error("monospace preview must be in a paragraph")
    return {
      paragraph: parseFloat(getComputedStyle(paragraph).fontSize),
      monospace: parseFloat(getComputedStyle(element).fontSize)
    }
  })
  assert.equal(sizes.paragraph, 16, "ordinary paragraph font size must remain 16px")
  assert.equal(
    sizes.monospace,
    sizes.paragraph * 0.98,
    "preview monospace font size must be 98% of the paragraph font size"
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 */
async function previewExisting(page, request, fixture, token) {
  const before = await readPage(request, fixture, fixture.existingSlug, token)
  assert.equal(before?.type, "found", "created local-create-proof fixture required")
  if (before.type !== "found") assert.fail("created local-create-proof fixture required")
  assert.ok(!before.data.wikitext.includes(fixture.rawMarker), "marker must be new")
  await openEditor(page, fixture.existingSlug)
  const title = `Preview ${fixture.rawMarker}`
  const source = `+ ${fixture.rawMarker}\n{{${fixture.rawMarker}}}\n**${fixture.rawMarker}**`
  await page.locator('#editor [name="title"]').fill(title)
  await fillFormattedSource(page, fixture.rawMarker)
  const { region, submitted } = await clickPreview(page, fixture.existingSlug)
  assert.equal(submitted.title, title, "preview must use typed title")
  assert.equal(submitted.wikitext, source, "preview must use typed source")
  assert.equal(submitted.last_revision_id, before.data.page_revision.revision_id)
  assert.ok(!Object.hasOwn(submitted, "form_updates"))
  await expect(region.locator("h1")).toContainText(fixture.rawMarker)
  await expect(region.locator("strong")).toContainText(fixture.rawMarker)
  await assertPreviewMonospaceSize(region, fixture.rawMarker)
  await assertStoredUnchanged(request, fixture, fixture.existingSlug, token, before.data)
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 */
async function previewMissing(page, request, fixture, token) {
  const before = await readPage(request, fixture, fixture.missingSlug, token)
  assert.equal(before?.type, "missing", "preview slug must not exist")
  await openEditor(page, fixture.missingSlug)
  const title = `Missing ${fixture.rawMarker}`
  const source = `+ ${fixture.rawMarker}\n{{${fixture.rawMarker}}}\n**${fixture.rawMarker}**`
  await page.locator('#editor [name="title"]').fill(title)
  await fillFormattedSource(page, fixture.rawMarker)
  const { region, submitted } = await clickPreview(page, fixture.missingSlug)
  assert.equal(submitted.title, title)
  assert.equal(submitted.wikitext, source)
  assert.ok(!submitted.last_revision_id, "new page must not claim a stored revision")
  await expect(region.locator("h1")).toContainText(fixture.rawMarker)
  await expect(region.locator("strong")).toContainText(fixture.rawMarker)
  await assertPreviewMonospaceSize(region, fixture.rawMarker)
  const after = await readPage(request, fixture, fixture.missingSlug, token)
  assert.equal(after?.type, "missing", "preview must not create missing page")
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 */
async function previewForm(page, request, fixture, token) {
  const before = await readPage(request, fixture, fixture.formSlug, token)
  assert.equal(before?.type, "found", "prepared data form must exist")
  if (before.type !== "found") assert.fail("prepared data form must exist")
  assert.ok(before.data.form, "form fixture required")
  const field = before.data.form.schema.fields.find(
    (candidate) => candidate.name === fixture.formFieldName
  )
  assert.ok(field, "form field must exist in schema")
  assert.equal(field.kind, "text", "Name preview requires a text field")
  assert.ok(
    !String(before.data.form.values[fixture.formFieldName]).includes(fixture.formMarker),
    "form marker must be new"
  )
  await openEditor(page, fixture.formSlug)
  await expect(page.locator('#editor [name="wikitext"]')).toHaveCount(0)
  await page
    .locator("#editor")
    .getByLabel(fixture.formFieldLabel, { exact: true })
    .fill(fixture.formMarker)
  const { region, submitted } = await clickPreview(page, fixture.formSlug)
  assert.deepEqual(submitted.form_updates, {
    [fixture.formFieldName]: fixture.formMarker
  })
  assert.ok(!Object.hasOwn(submitted, "wikitext"))
  assert.equal(submitted.last_revision_id, before.data.page_revision.revision_id)
  await expect(region).toContainText(fixture.formMarker)
  await assertStoredUnchanged(request, fixture, fixture.formSlug, token, before.data)
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {Fixture} fixture
 */
async function assertAnonymousPreviewDenied(context, fixture) {
  const response = await context.request.post(
    `${preview}/${fixture.existingSlug}?/preview`,
    {
      headers: { accept: "application/json", "x-sveltekit-action": "true" },
      form: { payload: JSON.stringify({ wikitext: `+ ${fixture.rawMarker}` }) }
    }
  )
  const action = await response.json()
  assert.equal(action.type, "failure", "anonymous preview action must reject access")
  assert.ok(action.status >= 400, "anonymous preview must fail")
  assert.ok(
    !Object.hasOwn(action.data ?? {}, "html"),
    "denial must not include rendered HTML"
  )
}

test("authenticated page previews render drafts without saving; anonymous preview is denied", async () => {
  const fixturePath = process.env.COBALT_PAGE_PREVIEW_FIXTURE
  const previewPasswordPath = process.env.COBALT_LOCAL_PASSWORD_FILE
  const adminPasswordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(
    fixturePath && previewPasswordPath && adminPasswordPath,
    "explicit preview fixture, Basic Auth password and account password files required"
  )
  const fixture = await readFixture(fixturePath)
  const previewPassword = (await readFile(previewPasswordPath, "utf8")).trim()
  const adminPassword = (await readFile(adminPasswordPath, "utf8")).trim()
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      httpCredentials: { username: "cobalt", password: previewPassword, origin: preview }
    })
    try {
      const { page, token } = await login(context, fixture, adminPassword)
      await previewExisting(page, context.request, fixture, token)
      await previewMissing(page, context.request, fixture, token)
      await previewForm(page, context.request, fixture, token)
    } finally {
      await context.close()
    }
    const anonymous = await browser.newContext({
      httpCredentials: { username: "cobalt", password: previewPassword, origin: preview }
    })
    try {
      await assertAnonymousPreviewDenied(anonymous, fixture)
    } finally {
      await anonymous.close()
    }
  } finally {
    await browser.close()
  }
})
