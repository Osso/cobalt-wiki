import assert from "node:assert/strict"
import { createHash, randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"

/**
 * @typedef {import("../../src/lib/server/deepwell/views").PageView} PageView
 *
 *
 * @typedef {{
 *   sacrificial: true
 *   siteId: 6000000
 *   siteSlug: "cobalt-company"
 *   databaseLabel: "cobalt_local_full"
 *   username: "cobalt-import"
 *   existingSlug: string
 *   formSlug: "local-acceptance:form-roundtrip"
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
  assert.equal(fixture.formSlug, "local-acceptance:form-roundtrip")
  return fixture
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 * @param {string} slug
 */
async function readPage(request, fixture, token, slug) {
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
  assert.equal(response.status(), 200, `${slug} page_view HTTP status`)
  const envelope = await response.json()
  assert.ok(!envelope.error, `${slug} page_view error ${envelope.error?.code ?? "?"}`)
  assert.ok(
    envelope.result && typeof envelope.result === "object",
    `${slug} page_view result`
  )
  return /** @type {PageView} */ (envelope.result)
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 * @param {string} slug
 */
async function readDraft(request, fixture, token, slug) {
  const response = await request.post(backend, {
    headers: {
      "X-Deepwell-Site-Id": String(fixture.siteId),
      "X-Deepwell-Page": slug,
      "X-Deepwell-Session-Token": token
    },
    data: { jsonrpc: "2.0", id: 1, method: "page_draft_get", params: {} }
  })
  assert.equal(response.status(), 200, `${slug} draft HTTP status`)
  const envelope = await response.json()
  assert.ok(!envelope.error, `${slug} draft error ${envelope.error?.code ?? "?"}`)
  assert.ok(Object.hasOwn(envelope.result ?? {}, "draft"), `${slug} draft response`)
  return envelope.result.draft
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {Fixture} fixture
 * @param {string} password
 */
async function login(context, fixture, password) {
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  await expect(page.locator("#login")).toBeVisible()
  await page.locator('#login [name="nameOrEmail"]').fill(fixture.username)
  await page.locator('#login [name="password"]').fill(password)
  const response = page.waitForResponse(
    (result) =>
      result.request().method() === "POST" &&
      new URL(result.url()).pathname === "/-/login"
  )
  await page.locator('#login button[type="submit"]').click()
  assert.equal((await response).status(), 200, "login action HTTP status")
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (entry) => entry.name === "wikijump_token" && entry.domain === "127.0.0.1"
  )
  assert.ok(cookie, "authenticated browser session required")
  return { page, token: decodeURIComponent(cookie.value) }
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} slug
 */
async function openEditor(page, slug) {
  await page.goto(`${origin}/${slug}/edit`, { waitUntil: "networkidle" })
  await expect(page.locator('#editor [name="title"]')).toBeVisible()
  await expect(page.locator('#editor [type="submit"]')).toBeEnabled()
  assert.equal(
    await page.getByRole("dialog", { name: "Saved draft" }).count(),
    0,
    "never restore or dismiss a pre-existing draft"
  )
}

/** @param {import("@playwright/test").Page} page */
async function countSubmits(page) {
  return page
    .locator("#editor")
    .evaluate((form) => Number(form.dataset.enterSubmitCount ?? 0))
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").Locator} field
 * @param {string} marker
 * @param {() => number} editCount
 */
async function assertImplicitEnterBlocked(page, field, marker, editCount) {
  await field.fill(marker)
  await field.focus()
  const before = await countSubmits(page)
  const requests = editCount()
  await page.keyboard.press("Enter")
  assert.equal(
    await countSubmits(page),
    before,
    "single-line Enter must not submit editor"
  )
  assert.equal(editCount(), requests, "single-line Enter must not request edit action")
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").Locator} field
 * @param {string} marker
 * @param {() => number} editCount
 */
async function assertTextareaEnter(page, field, marker, editCount) {
  await field.fill(marker)
  await field.focus()
  const before = await countSubmits(page)
  const requests = editCount()
  await page.keyboard.press("Enter")
  await expect(field).toHaveValue(`${marker}\n`)
  assert.equal(await countSubmits(page), before, "textarea Enter must not submit editor")
  assert.equal(editCount(), requests, "textarea Enter must not request edit action")
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").Locator} field
 */
async function assertCompositionNotPrevented(page, field) {
  const prevented = await field.evaluate((element) => {
    const event = new KeyboardEvent("keydown", {
      key: "Enter",
      bubbles: true,
      cancelable: true,
      isComposing: true
    })
    return !element.dispatchEvent(event)
  })
  assert.equal(prevented, false, "synthetic composing Enter must not be prevented")
}

/** @param {unknown} value */
function digest(value) {
  return createHash("sha256").update(JSON.stringify(value)).digest("hex")
}

/** @param {PageView} view */
function pageSnapshot(view) {
  if (view.type !== "found") return { type: view.type }
  return {
    type: view.type,
    revisionId: view.data.page_revision.revision_id,
    sourceSha256: createHash("sha256").update(view.data.wikitext).digest("hex"),
    revisionHash: digest(view.data.page_revision.wikitext_hash),
    formValuesSha256: digest(view.data.form?.values ?? null)
  }
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 * @param {{ slug: string; present: boolean; form: boolean }} target
 */
async function readBaseline(request, fixture, token, target) {
  const { slug, present, form } = target
  const before = await readPage(request, fixture, token, slug)
  assert.equal(before.type, present ? "found" : "missing", `${slug} initial existence`)
  if (before.type === "found") {
    assert.equal(Boolean(before.data.form), form, `${slug} source kind`)
    assert.equal(typeof before.data.wikitext, "string", `${slug} stored source baseline`)
    assert.ok(before.data.page_revision?.revision_id, `${slug} revision baseline`)
    assert.ok(
      before.data.page_revision.wikitext_hash?.length,
      `${slug} source revision hash`
    )
  }
  assert.ok(
    (await readDraft(request, fixture, token, slug)) === null,
    `${slug} initial draft`
  )
  return pageSnapshot(before)
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 */
async function readFormLabels(request, fixture, token) {
  const schemaPage = await readPage(request, fixture, token, fixture.formSlug)
  assert.equal(schemaPage.type, "found", "prepared form schema required")
  if (schemaPage.type !== "found") assert.fail("prepared form schema required")
  const fields = schemaPage.data.form?.schema.fields
  assert.ok(fields, "prepared form schema fields required")
  for (const [name, kind] of [
    ["name", "text"],
    ["count", "text"],
    ["notes", "wiki"],
    ["kind", "select"],
    ["region", "select"]
  ]) {
    assert.equal(
      fields.find((field) => field.name === name)?.kind,
      kind,
      `${name} form field`
    )
  }
  /** @param {string} name */
  return (name) => {
    const field = fields.find((candidate) => candidate.name === name)
    assert.ok(field, `${name} form field required`)
    return String(field.properties.label || field.name)
  }
}

/** @param {import("@playwright/test").Page} page */
async function observeSubmits(page) {
  await page.locator("#editor").evaluate((element) => {
    element.addEventListener(
      "submit",
      (event) => {
        if (event.target !== element) return
        element.dataset.enterSubmitCount = String(
          Number(element.dataset.enterSubmitCount ?? 0) + 1
        )
      },
      true
    )
  })
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {boolean} form
 * @param {(name: string) => string} label
 */
async function assertEditorFields(page, form, label) {
  const editor = page.locator("#editor")
  if (!form) {
    await expect(editor.locator('[name="wikitext"]')).toBeVisible()
    return
  }
  await expect(editor.locator('[name="wikitext"]')).toHaveCount(0)
  await expect(editor.getByLabel(label("name"), { exact: true })).toBeVisible()
  await expect(editor.getByLabel(label("notes"), { exact: true })).toBeVisible()
}

/** @param {import("@playwright/test").Page} page */
async function assertStructuredFooterGeometry(page) {
  const editor = page.locator("#editor")
  const table = editor.getByRole("table")
  const cancel = editor.getByRole("button", { name: "Cancel", exact: true })
  const save = editor.getByRole("button", { name: "Save", exact: true })
  for (const control of [table, cancel, save]) await expect(control).toBeVisible()
  const tableBox = await table.boundingBox()
  const cancelBox = await cancel.boundingBox()
  const saveBox = await save.boundingBox()
  assert.ok(tableBox && cancelBox && saveBox, "structured footer rectangles required")
  const leftOffset = cancelBox.x - tableBox.x
  assert.ok(
    Math.abs(leftOffset - 2) <= 2,
    `Cancel must sit 2 ± 2 px from form table left; got ${leftOffset}`
  )
  const centerOffset = cancelBox.y + cancelBox.height / 2 - saveBox.y - saveBox.height / 2
  assert.ok(
    Math.abs(centerOffset) <= 2,
    `Cancel and Save must share a line; center offset ${centerOffset} px`
  )
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {string} slug
 * @param {{ url: string; edit: boolean }[]} blocked
 */
async function trapPagePosts(context, slug, blocked) {
  await context.route("**/*", async (route) => {
    const outgoing = route.request()
    if (outgoing.method() !== "POST") return route.continue()
    const url = new URL(outgoing.url())
    blocked.push({
      url: `${url.origin}${url.pathname}${url.search}`,
      edit:
        url.origin === origin &&
        url.pathname === `/${slug}/edit` &&
        url.search === "?/edit"
    })
    await route.abort()
  })
}

/**
 * @param {{ url: string; edit: boolean }[]} blocked
 * @param {number} expected
 * @param {string} slug
 */
function assertOnlySavePost(blocked, expected, slug) {
  assert.deepEqual(
    blocked.filter((entry) => !entry.edit),
    [],
    `${slug} unexpected page POSTs`
  )
  assert.equal(
    blocked.filter((entry) => entry.edit).length,
    expected,
    `${slug} edit POST count`
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {boolean} form
 * @param {(name: string) => string} label
 * @param {() => number} editCount
 */
async function assertInputEnter(page, form, label, editCount) {
  const editor = page.locator("#editor")
  const marker = `Enter proof ${randomBytes(6).toString("hex")}`
  const title = editor.locator('[name="title"]')
  const fields = [title, editor.locator('[name="tags"]')]
  if (form) fields.push(editor.getByLabel(label("name"), { exact: true }))
  for (const field of fields) {
    await assertImplicitEnterBlocked(page, field, marker, editCount)
  }
  const textarea = form
    ? editor.getByLabel(label("notes"), { exact: true })
    : editor.locator('[name="wikitext"]')
  await assertTextareaEnter(page, textarea, marker, editCount)
  await assertCompositionNotPrevented(page, title)
  if (form) {
    await assertRadioEnterBlocked(page, editCount)
  }
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {() => number} editCount
 */
async function assertRadioEnterBlocked(page, editCount) {
  const radio = page.locator("#editor").getByRole("radio").first()
  await expect(radio).toBeVisible()
  const submits = await countSubmits(page)
  const requests = editCount()
  await radio.press("Enter")
  assert.equal(await countSubmits(page), submits, "radio Enter must not submit editor")
  assert.equal(editCount(), requests, "radio Enter must not request edit action")
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {() => number} editCount
 */
async function assertUrlWizardEnter(page, editCount) {
  const editor = page.locator("#editor")
  const source = editor.locator('[name="wikitext"]')
  const prefix = "Wizard Enter proof "
  const uri = `${origin}/cobalt-editor/icons1.png`
  const anchor = "Synthetic anchor"
  await source.fill(prefix)
  await page
    .getByRole("toolbar", { name: "Wikitext formatting" })
    .getByRole("button", { name: "URL link wizard", exact: true })
    .click()
  const dialog = page.getByRole("dialog", { name: "URL link wizard" })
  await expect(dialog).toBeVisible()
  await dialog.getByLabel("URL:").fill(uri)
  const anchorInput = dialog.getByLabel("Anchor text:")
  await anchorInput.fill(anchor)
  const ownsWizardForm = await anchorInput.evaluate((element) => {
    if (!(element instanceof HTMLInputElement)) throw new Error("anchor input required")
    const editorForm = document.querySelector("#editor")
    if (!(editorForm instanceof HTMLFormElement)) throw new Error("editor form required")
    return (
      element.form instanceof HTMLFormElement &&
      element.form !== editorForm &&
      element.form.closest("dialog") === element.closest("dialog")
    )
  })
  assert.ok(ownsWizardForm, "anchor input must belong to wizard form, not editor")
  const submits = await countSubmits(page)
  const requests = editCount()
  await anchorInput.press("Enter")
  await expect(dialog).toHaveCount(0)
  await expect(source).toHaveValue(`${prefix}[${uri} ${anchor}]`)
  assert.equal(await countSubmits(page), submits, "wizard Enter must not submit editor")
  assert.equal(editCount(), requests, "wizard Enter must not request edit action")
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} slug
 * @param {() => number} editCount
 */
async function assertSaveEnter(page, slug, editCount) {
  const editor = page.locator("#editor")
  const submits = await countSubmits(page)
  const requestPromise = page.waitForRequest(
    (outgoing) =>
      outgoing.method() === "POST" &&
      new URL(outgoing.url()).pathname === `/${slug}/edit` &&
      new URL(outgoing.url()).search === "?/edit"
  )
  await editor.locator('[type="submit"]').focus()
  await page.keyboard.press("Enter")
  await requestPromise
  await expect.poll(editCount).toBe(1)
  assert.equal(
    await countSubmits(page),
    submits + 1,
    "Save keyboard activation submits once"
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {() => number} editCount
 */
async function assertCancelEnter(page, editCount) {
  const editor = page.locator("#editor")
  const cancel = editor.getByRole("button", { name: "Cancel", exact: true })
  await expect(cancel).toBeVisible()
  await cancel.focus()
  await page.keyboard.press("Enter")
  await expect(editor).toHaveCount(0)
  await expect(page).not.toHaveURL(/\/edit(?:\?|$)/)
  assert.equal(editCount(), 1, "Cancel keyboard activation must not save")
  await expect(page.getByRole("dialog", { name: "Keep saved draft" })).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 * @param {string} slug
 * @param {ReturnType<typeof pageSnapshot>} before
 * @param {{ url: string; edit: boolean }[]} blocked
 * @param {number} expectedSaves
 */
async function assertReadback(
  request,
  fixture,
  token,
  slug,
  before,
  blocked,
  expectedSaves
) {
  const after = pageSnapshot(await readPage(request, fixture, token, slug))
  assert.equal(after.type, before.type, `${slug} existence preserved`)
  if (before.type === "found" && after.type === "found") {
    assert.equal(after.revisionId, before.revisionId, `${slug} revision ID preserved`)
    assert.equal(
      after.sourceSha256,
      before.sourceSha256,
      `${slug} source SHA256 preserved`
    )
    assert.equal(
      after.revisionHash,
      before.revisionHash,
      `${slug} source revision hash preserved`
    )
    assert.equal(
      after.formValuesSha256,
      before.formValuesSha256,
      `${slug} form values SHA256 preserved`
    )
  }
  assert.ok(
    (await readDraft(request, fixture, token, slug)) === null,
    `${slug} draft absent`
  )
  assertOnlySavePost(blocked, expectedSaves, slug)
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {Fixture} fixture
 * @param {string} token
 * @param {{ slug: string; present: boolean; form: boolean }} target
 */
async function checkTarget(context, fixture, token, target) {
  const { slug, form } = target
  const page = await context.newPage()
  /** @type {{ url: string; edit: boolean }[]} */
  const blocked = []
  let trapped = false
  let expectedSaves = 0
  let before
  try {
    before = await readBaseline(context.request, fixture, token, target)
    const label = form ? await readFormLabels(context.request, fixture, token) : () => ""
    await openEditor(page, slug)
    await trapPagePosts(context, slug, blocked)
    trapped = true
    await assertEditorFields(page, form, label)
    if (form) await assertStructuredFooterGeometry(page)
    await observeSubmits(page)
    const editCount = () => blocked.filter((entry) => entry.edit).length
    await assertInputEnter(page, form, label, editCount)
    if (!form) await assertUrlWizardEnter(page, editCount)
    assertOnlySavePost(blocked, 0, slug)
    await assertSaveEnter(page, slug, editCount)
    expectedSaves = 1
    assertOnlySavePost(blocked, 1, slug)
    await assertCancelEnter(page, editCount)
  } finally {
    try {
      await page.close()
      if (before && trapped) {
        await assertReadback(
          context.request,
          fixture,
          token,
          slug,
          before,
          blocked,
          expectedSaves
        )
      }
    } finally {
      if (trapped) await context.unroute("**/*")
    }
  }
}

test("editor Enter safety on existing and missing raw and form pages", async (t) => {
  const fixturePath = process.env.COBALT_PAGE_PREVIEW_FIXTURE
  const passwordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(
    fixturePath && passwordPath,
    "protected preview fixture and admin password file required"
  )
  const fixture = await readFixture(fixturePath)
  const password = (await readFile(passwordPath, "utf8")).trim()
  assert.ok(password, "admin password file must not be empty")
  const suffix = randomBytes(8).toString("hex")
  const targets = [
    { slug: fixture.existingSlug, present: true, form: false },
    { slug: fixture.formSlug, present: true, form: true },
    { slug: `local-preview-proof:${suffix}`, present: false, form: false },
    { slug: `local-acceptance:enter-${suffix}`, present: false, form: true }
  ]
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext()
    try {
      const { page, token } = await login(context, fixture, password)
      await page.close()
      for (const target of targets) {
        await t.test(
          `${target.present ? "existing" : "missing"} ${target.form ? "form" : "raw"}`,
          async () => {
            await checkTarget(context, fixture, token, target)
          }
        )
      }
    } finally {
      await context.close()
    }
  } finally {
    await browser.close()
  }
})
