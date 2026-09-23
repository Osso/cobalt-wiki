import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const preview = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000

/**
 * @typedef {import("../../src/lib/server/deepwell/views").PageView} PageView
 *
 *
 * @typedef {Extract<PageView, { type: "found" }>["data"]} StoredPage
 *
 * @typedef {import("../../src/lib/server/deepwell/page-draft").PageDraft} PageDraft
 *
 *
 * @typedef {{
 *   sacrificial: true
 *   siteSlug: "cobalt-company"
 *   databaseLabel: "cobalt_local_full"
 *   siteId: 6000000
 *   username: "cobalt-import"
 *   rawSlug: string
 *   formSlug: string
 *   draftMarker: string
 *   formMarker: string
 * }} Fixture
 */

/** @param {string} path */
async function readFixture(path) {
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(path, "utf8"))
  assert.equal(fixture.sacrificial, true, "sacrificial fixture required")
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.siteId, siteId)
  assert.equal(fixture.username, "cobalt-import")
  assert.match(fixture.rawSlug, /^local-draft-proof:roundtrip-[a-f0-9]{12}$/)
  assert.match(fixture.formSlug, /^local-acceptance:draft-roundtrip-[a-f0-9]{12}$/)
  assert.match(fixture.draftMarker, /^[a-z][a-z0-9-]{15,}$/)
  assert.match(fixture.formMarker, /^[a-z][a-z0-9-]{15,}$/)
  assert.notEqual(fixture.draftMarker, fixture.formMarker)
  return fixture
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string | null} token
 * @param {string} slug
 * @param {string} method
 * @param {unknown} params
 */
async function sendRpc(request, token, slug, method, params) {
  /** @type {Record<string, string>} */
  const headers = {
    "X-Deepwell-Site-Id": String(siteId),
    "X-Deepwell-Page": slug
  }
  if (token) headers["X-Deepwell-Session-Token"] = token
  const response = await request.post(backend, {
    headers,
    data: { jsonrpc: "2.0", id: 1, method, params }
  })
  assert.equal(response.status(), 200, `${method} HTTP status`)
  return response.json()
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string | null} token
 * @param {string} slug
 * @param {string} method
 * @param {unknown} params
 */
async function rpc(request, token, slug, method, params) {
  const envelope = await sendRpc(request, token, slug, method, params)
  assert.ok(!envelope.error, `${method} RPC error code ${envelope.error?.code ?? "?"}`)
  return envelope.result
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string | null} token
 * @param {string} slug
 */
async function readPage(request, token, slug) {
  /** @type {PageView} */
  const view = await rpc(request, token, slug, "page_view", {
    site_id: siteId,
    locales: ["en"],
    session_token: token,
    route: { slug, extra: "" }
  })
  return view
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {string} slug
 */
async function readDraft(request, token, slug) {
  /** @type {{ draft: PageDraft | null }} */
  const result = await rpc(request, token, slug, "page_draft_get", {})
  return result.draft
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {string} slug
 */
async function readStoredPage(request, token, slug) {
  const view = await readPage(request, token, slug)
  assert.equal(view.type, "found", `${slug} must be published`)
  if (view.type !== "found") assert.fail(`${slug} must be published`)
  return view.data
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
  assert.ok(cookie, "login must establish browser session")
  return { page, token: decodeURIComponent(cookie.value) }
}

/** @param {import("@playwright/test").Page} page @param {string} slug */
async function openEditor(page, slug) {
  await page.goto(`${preview}/${slug}/edit`, { waitUntil: "networkidle" })
  await expect(page.locator("#editor")).toBeVisible()
  await expect(page.locator("#edit-save-draft-button")).toBeVisible()
}

/** @param {import("@playwright/test").Page} page @param {string} label */
async function chooseSavedDraft(page, label) {
  const dialog = page.getByRole("dialog", { name: "Saved draft" })
  await expect(dialog).toBeVisible()
  await expect(page.locator('#editor [type="submit"]')).toBeDisabled()
  await dialog.getByRole("button", { name: label, exact: true }).click()
  await expect(dialog).toHaveCount(0)
}

/** @param {import("@playwright/test").Page} page */
async function saveDraft(page) {
  await page.locator("#edit-save-draft-button").click()
  await expect(page.getByRole("status")).toContainText("Draft saved")
}

/** @param {import("@playwright/test").Page} page */
async function publish(page) {
  const responsePromise = page.waitForResponse(
    (response) =>
      response.request().method() === "POST" && response.url().includes("?/edit")
  )
  await page.locator('#editor [type="submit"]').click()
  assert.equal((await responsePromise).status(), 200, "publish action HTTP status")
  await expect(page.locator("#editor")).toHaveCount(0)
}

/** @param {import("@playwright/test").Page} page @param {string} label */
async function cancelWithChoice(page, label) {
  await page
    .locator("#editor")
    .getByRole("button", { name: "Cancel", exact: true })
    .click()
  const dialog = page.getByRole("dialog", { name: "Keep saved draft" })
  await expect(dialog).toBeVisible()
  await dialog.getByRole("button", { name: label, exact: true }).click()
  await expect(dialog).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 */
async function seedFreshForm(request, fixture, token) {
  const template = await readStoredPage(request, token, "local-acceptance:_template")
  assert.ok(template.wikitext, "form template source required")
  const missing = await readPage(request, token, fixture.formSlug)
  assert.equal(missing.type, "missing", "fresh form slug must be absent")
  const session = await rpc(request, token, fixture.formSlug, "session_get", [token])
  assert.ok(Number.isSafeInteger(session?.user_id), "authenticated actor required")
  const initialValues = {
    name: "Draft before",
    count: 3,
    notes: "Before notes",
    kind: "a",
    region: "north",
    unknown: true
  }
  const created = await rpc(request, token, fixture.formSlug, "page_create", {
    site_id: siteId,
    user_id: session.user_id,
    ip_address: "127.0.0.1",
    slug: fixture.formSlug,
    title: `Form ${fixture.formMarker}`,
    wikitext: JSON.stringify(initialValues),
    tags: [],
    revision_comments: "Local draft acceptance form seed"
  })
  assert.equal(created.slug, fixture.formSlug)
  const published = await readStoredPage(request, token, fixture.formSlug)
  assert.deepEqual(published.form?.values, initialValues)
  const names = published.form.schema.fields.map((field) => field.name)
  for (const name of ["name", "count", "notes", "kind", "region"]) {
    assert.ok(names.includes(name), `${name} must be in form template`)
  }
  assert.ok(!names.includes("unknown"), "unknown value must be outside schema")
  return { published, initialValues }
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 */
async function assertAnonymousDenied(context, request, fixture) {
  const source = `Private ${fixture.draftMarker}`
  for (const method of ["page_draft_get", "page_draft_save", "page_draft_delete"]) {
    const params =
      method === "page_draft_save"
        ? { title: source, wikitext: source, user_id: 1, site_id: siteId }
        : { user_id: 1, site_id: siteId }
    const envelope = await sendRpc(request, null, fixture.rawSlug, method, params)
    assert.ok(envelope.error, `anonymous ${method} RPC must fail`)
    assert.ok(!Object.hasOwn(envelope, "result"), "denied RPC must have no result")
    const serialized = JSON.stringify(envelope)
    assert.ok(
      !serialized.includes(fixture.draftMarker),
      "RPC failure must not leak draft"
    )
  }
  for (const action of ["draftGet", "draftSave", "draftDelete"]) {
    /** @type {Record<string, string>} */
    const form =
      action === "draftSave"
        ? { payload: JSON.stringify({ title: source, wikitext: source }) }
        : {}
    const response = await context.request.post(
      `${preview}/${fixture.rawSlug}/edit?/${action}`,
      {
        headers: { accept: "application/json", "x-sveltekit-action": "true" },
        form
      }
    )
    const envelope = await response.json()
    assert.equal(envelope.type, "failure", `anonymous ${action} must fail`)
    assert.ok(envelope.status >= 400)
    assert.ok(
      !JSON.stringify(envelope).includes(fixture.draftMarker),
      "action must not leak draft"
    )
  }
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 * @param {import("@playwright/test").BrowserContext} anonymous
 */
async function exerciseRawDraft(page, request, fixture, token, anonymous) {
  const title = `Draft ${fixture.draftMarker}`
  const source = `+ ${fixture.draftMarker}\nUnpublished source\n`
  assert.equal((await readPage(request, token, fixture.rawSlug)).type, "missing")
  assert.equal(await readDraft(request, token, fixture.rawSlug), null)
  await openEditor(page, fixture.rawSlug)
  await page.locator('#editor [name="title"]').fill(title)
  await page.locator('#editor [name="wikitext"]').fill(source)
  await saveDraft(page)
  assert.equal((await readPage(request, token, fixture.rawSlug)).type, "missing")
  const saved = await readDraft(request, token, fixture.rawSlug)
  assert.equal(saved?.title, title)
  assert.equal(saved?.wikitext, source)
  const search = await rpc(request, token, fixture.rawSlug, "page_search", {
    query: fixture.draftMarker,
    offset: 0,
    limit: 20
  })
  assert.deepEqual(search.hits, [], "unpublished marker must not be indexed")
  await assertAnonymousDenied(anonymous, anonymous.request, fixture)

  await cancelWithChoice(page, "Leave Draft")
  await expect(page.locator("#editor")).toHaveCount(0)
  assert.equal((await readDraft(request, token, fixture.rawSlug))?.wikitext, source)
  await openEditor(page, fixture.rawSlug)
  await chooseSavedDraft(page, "Edit Draft")
  await expect(page.locator('#editor [name="title"]')).toHaveValue(title)
  await expect(page.locator('#editor [name="wikitext"]')).toHaveValue(source)
  await publish(page)
  const published = await readStoredPage(request, token, fixture.rawSlug)
  assert.equal(published.page_revision.title, title)
  assert.equal(published.wikitext, source)
  assert.equal(await readDraft(request, token, fixture.rawSlug), null)
  return published
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 * @param {StoredPage} original
 */
async function exerciseExistingRawDraft(page, request, fixture, token, original) {
  await openEditor(page, fixture.rawSlug)
  await page.locator('#editor [name="title"]').fill(`Replacement ${fixture.draftMarker}`)
  await page
    .locator('#editor [name="wikitext"]')
    .fill(`Replacement source ${fixture.draftMarker}`)
  await saveDraft(page)
  assert.equal(
    (await readDraft(request, token, fixture.rawSlug))?.wikitext,
    `Replacement source ${fixture.draftMarker}`
  )
  const unchanged = await readStoredPage(request, token, fixture.rawSlug)
  assert.equal(unchanged.page_revision.revision_id, original.page_revision.revision_id)
  assert.equal(unchanged.wikitext, original.wikitext)
  await page.reload({ waitUntil: "networkidle" })
  await chooseSavedDraft(page, "Edit Original")
  await expect(page.locator('#editor [name="wikitext"]')).toHaveValue(original.wikitext)
  await publish(page)
  const published = await readStoredPage(request, token, fixture.rawSlug)
  assert.equal(published.wikitext, original.wikitext)
  assert.equal(published.page_revision.title, original.page_revision.title)
  assert.equal(await readDraft(request, token, fixture.rawSlug), null)

  await openEditor(page, fixture.rawSlug)
  await page.locator('#editor [name="wikitext"]').fill(`Discard ${fixture.draftMarker}`)
  await saveDraft(page)
  await cancelWithChoice(page, "Delete Draft")
  await expect(page.locator("#editor")).toHaveCount(0)
  assert.equal(await readDraft(request, token, fixture.rawSlug), null)
  const afterDelete = await readStoredPage(request, token, fixture.rawSlug)
  assert.equal(afterDelete.page_revision.revision_id, published.page_revision.revision_id)
  assert.equal(afterDelete.wikitext, original.wikitext)
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 * @param {StoredPage} original
 * @param {Record<string, string | number | boolean | null>} initialValues
 */
async function exerciseFormDraft(page, request, fixture, token, original, initialValues) {
  const name = `Draft ${fixture.formMarker}`
  const notes = `Published ${fixture.formMarker}`
  const form = original.form
  assert.ok(form, "form schema required")
  /** @param {string} key */
  const fieldLabel = (key) => {
    const field = form.schema.fields.find((candidate) => candidate.name === key)
    assert.ok(field, `${key} field required`)
    return String(field.properties.label || field.name)
  }
  await openEditor(page, fixture.formSlug)
  const editor = page.locator("#editor")
  await editor.getByLabel(fieldLabel("name"), { exact: true }).fill(name)
  await saveDraft(page)
  const saved = await readDraft(request, token, fixture.formSlug)
  assert.deepEqual(saved?.form_values, { ...initialValues, name })
  const unchanged = await readStoredPage(request, token, fixture.formSlug)
  assert.equal(unchanged.page_revision.revision_id, original.page_revision.revision_id)
  assert.equal(unchanged.wikitext, original.wikitext)
  assert.deepEqual(unchanged.form?.values, initialValues)
  await cancelWithChoice(page, "Leave Draft")
  await expect(editor).toHaveCount(0)
  await openEditor(page, fixture.formSlug)
  await chooseSavedDraft(page, "Edit Draft")
  await expect(editor.getByLabel(fieldLabel("name"), { exact: true })).toHaveValue(name)
  await editor.getByLabel(fieldLabel("notes"), { exact: true }).fill(notes)
  await publish(page)
  const published = await readStoredPage(request, token, fixture.formSlug)
  assert.deepEqual(published.form?.values, { ...initialValues, name, notes })
  assert.deepEqual(JSON.parse(published.wikitext), { ...initialValues, name, notes })
  assert.equal(await readDraft(request, token, fixture.formSlug), null)
}

test("authenticated browser draft lifecycle preserves unpublished and typed form state", async () => {
  const fixturePath = process.env.COBALT_PAGE_DRAFT_FIXTURE
  const previewPasswordPath = process.env.COBALT_LOCAL_PASSWORD_FILE
  const adminPasswordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(
    fixturePath && previewPasswordPath && adminPasswordPath,
    "explicit sacrificial fixture, preview password and admin password files required"
  )
  const fixture = await readFixture(fixturePath)
  const previewPassword = (await readFile(previewPasswordPath, "utf8")).trim()
  const adminPassword = (await readFile(adminPasswordPath, "utf8")).trim()
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const anonymous = await browser.newContext({
      httpCredentials: { username: "cobalt", password: previewPassword, origin: preview }
    })
    const context = await browser.newContext({
      httpCredentials: { username: "cobalt", password: previewPassword, origin: preview }
    })
    try {
      const { page, token } = await login(context, fixture, adminPassword)
      const original = await exerciseRawDraft(
        page,
        context.request,
        fixture,
        token,
        anonymous
      )
      await exerciseExistingRawDraft(page, context.request, fixture, token, original)
      const form = await seedFreshForm(context.request, fixture, token)
      await exerciseFormDraft(
        page,
        context.request,
        fixture,
        token,
        form.published,
        form.initialValues
      )
    } finally {
      await context.close()
      await anonymous.close()
    }
  } finally {
    await browser.close()
  }
})
