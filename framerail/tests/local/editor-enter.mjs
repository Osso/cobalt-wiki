import assert from "node:assert/strict"
import { randomBytes } from "node:crypto"
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

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {import("@playwright/test").Page} page
 * @param {Fixture} fixture
 * @param {string} token
 * @param {{ slug: string; present: boolean; form: boolean }} target
 */
async function checkTarget(context, page, fixture, token, target) {
  const { slug, present, form } = target
  const request = context.request
  const before = await readPage(request, fixture, token, slug)
  assert.equal(before.type, present ? "found" : "missing", `${slug} initial existence`)
  if (present && before.type === "found") {
    assert.equal(Boolean(before.data.form), form, `${slug} source kind`)
    assert.equal(typeof before.data.wikitext, "string", `${slug} stored source baseline`)
    assert.ok(before.data.page_revision?.revision_id, `${slug} revision baseline`)
    assert.ok(
      before.data.page_revision.wikitext_hash?.length > 0,
      `${slug} source revision hash baseline`
    )
  }
  const schemaPage = form
    ? await readPage(request, fixture, token, fixture.formSlug)
    : null
  if (form) {
    assert.equal(schemaPage?.type, "found", "prepared form schema required")
    if (schemaPage?.type !== "found") assert.fail("prepared form schema required")
    for (const [name, kind] of [
      ["name", "text"],
      ["count", "text"],
      ["notes", "wiki"],
      ["kind", "select"],
      ["region", "select"]
    ]) {
      assert.equal(
        schemaPage.data.form?.schema.fields.find((field) => field.name === name)?.kind,
        kind,
        `${slug} ${name} form field`
      )
    }
  }
  /** @param {string} name */
  const label = (name) => {
    const field =
      schemaPage?.type === "found"
        ? schemaPage.data.form?.schema.fields.find((candidate) => candidate.name === name)
        : undefined
    assert.ok(field, `${slug} ${name} form field required`)
    return String(field.properties.label || field.name)
  }
  assert.equal(
    await readDraft(request, fixture, token, slug),
    null,
    `${slug} must have no draft before edit`
  )
  /** @type {{ url: string; edit: boolean }[]} */
  const blocked = []
  let intercepting = false
  try {
    await openEditor(page, slug)
    const editor = page.locator("#editor")
    if (form) {
      await expect(editor.locator('[name="wikitext"]')).toHaveCount(0)
      await expect(editor.getByLabel(label("name"), { exact: true })).toBeVisible()
      await expect(editor.getByLabel(label("notes"), { exact: true })).toBeVisible()
    } else {
      await expect(editor.locator('[name="wikitext"]')).toBeVisible()
    }
    await editor.evaluate((element) => {
      element.addEventListener(
        "submit",
        () => {
          element.dataset.enterSubmitCount = String(
            Number(element.dataset.enterSubmitCount ?? 0) + 1
          )
        },
        true
      )
    })
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
    intercepting = true
    const editCount = () => blocked.filter((entry) => entry.edit).length
    const marker = `Enter proof ${randomBytes(6).toString("hex")}`
    const title = editor.locator('[name="title"]')
    await assertImplicitEnterBlocked(page, title, marker, editCount)
    if (form) {
      await assertImplicitEnterBlocked(
        page,
        editor.getByLabel(label("name"), { exact: true }),
        marker,
        editCount
      )
    }
    const textarea = form
      ? editor.getByLabel(label("notes"), { exact: true })
      : editor.locator('[name="wikitext"]')
    await assertTextareaEnter(page, textarea, marker, editCount)
    await assertCompositionNotPrevented(page, title)
    assert.deepEqual(
      blocked.filter((entry) => !entry.edit),
      [],
      "unexpected page POSTs"
    )
    assert.equal(editCount(), 0, "no edit POST before explicit Save")
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
    assert.deepEqual(
      blocked.filter((entry) => !entry.edit),
      [],
      "unexpected page POSTs"
    )
    const cancel = editor.getByRole("button", { name: "Cancel", exact: true })
    if (await cancel.isVisible()) {
      await cancel.focus()
      await page.keyboard.press("Enter")
      await expect(editor).toHaveCount(0)
      assert.equal(editCount(), 1, "Cancel keyboard activation must not save")
      assert.equal(
        await page.getByRole("dialog", { name: "Keep saved draft" }).count(),
        0
      )
    }
  } finally {
    if (intercepting) await context.unroute("**/*")
    const after = await readPage(request, fixture, token, slug)
    assert.equal(after.type, before.type, `${slug} existence preserved`)
    if (before.type === "found" && after.type === "found") {
      assert.deepEqual(
        after.data.page_revision,
        before.data.page_revision,
        `${slug} revision preserved`
      )
      assert.equal(after.data.wikitext, before.data.wikitext, `${slug} source preserved`)
      assert.deepEqual(
        after.data.page_revision.wikitext_hash,
        before.data.page_revision.wikitext_hash,
        `${slug} source revision hash preserved`
      )
      assert.deepEqual(
        after.data.form?.values,
        before.data.form?.values,
        `${slug} form values preserved`
      )
    }
    assert.equal(
      await readDraft(request, fixture, token, slug),
      null,
      `${slug} draft preserved absent`
    )
    assert.deepEqual(
      blocked.filter((entry) => !entry.edit),
      [],
      `${slug} unexpected page POSTs`
    )
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
      for (const target of targets) {
        await t.test(
          `${target.present ? "existing" : "missing"} ${target.form ? "form" : "raw"}`,
          async () => {
            await checkTarget(context, page, fixture, token, target)
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
