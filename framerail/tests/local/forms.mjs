import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const preview = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"

/** @param {unknown} value */
const digest = (value) => createHash("sha256").update(JSON.stringify(value)).digest("hex")

/**
 * @typedef {string | number | boolean | null} Scalar
 *
 * @typedef {{
 *   name: string
 *   label: string
 *   initial: Scalar
 *   next: Scalar
 *   nextLabel?: string
 * }} Field
 *
 *
 * @typedef {{
 *   sacrificial: true
 *   siteSlug: "cobalt-company"
 *   databaseLabel: "cobalt_local_full"
 *   siteId: 6000000
 *   slug: "local-acceptance:form-roundtrip"
 *   username: "cobalt-import"
 *   fields: { text: Field; wiki: Field; radio: Field; select: Field }
 *   initialValues: Record<string, Scalar>
 *   expectedValues: Record<string, Scalar>
 *   unchangedKeys: string[]
 *   unknownKeys: string[]
 * }} Fixture
 */

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {Fixture} fixture @param {string | null} sessionToken
 */
async function readPage(request, fixture, sessionToken) {
  const response = await request.post(backend, {
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "page_view",
      params: {
        site_id: fixture.siteId,
        locales: ["en"],
        session_token: sessionToken,
        route: { slug: fixture.slug, extra: "" }
      }
    }
  })
  assert.equal(response.status(), 200, "local backend page_view must respond")
  const payload = await response.json()
  assert.ok(!payload.error, `page_view failed (code ${payload.error?.code ?? "?"})`)
  assert.equal(payload.result?.type, "found", "dedicated fixture page must exist")
  assert.ok(payload.result.data.form, "dedicated fixture must be a data form")
  return payload.result.data
}

/**
 * @param {Record<string, Scalar>} actual @param {Record<string, Scalar>}
 *   expected @param {string} stage
 */
function assertValues(actual, expected, stage) {
  assert.equal(
    Object.keys(actual).length,
    Object.keys(expected).length,
    `${stage} scalar count`
  )
  assert.deepEqual(
    Object.keys(actual).sort(),
    Object.keys(expected).sort(),
    `${stage} scalar keys`
  )
  for (const key of Object.keys(expected)) {
    assert.equal(
      digest(actual[key]),
      digest(expected[key]),
      `${stage} value digest for ${key}`
    )
  }
}

/**
 * @param {import("@playwright/test").Page} page @param {Fixture} fixture
 * @param {"initial" | "next"} version
 */
async function assertControls(page, fixture, version) {
  const editor = page.locator("#editor")
  await expect(editor).toBeVisible()
  await expect(editor.locator(".editor-wikitext")).toHaveCount(0)
  for (const kind of ["text", "wiki"]) {
    const field = fixture.fields[kind]
    await expect(editor.getByLabel(field.label, { exact: true })).toHaveValue(
      String(field[version])
    )
  }
  const radio = editor.getByRole("group", { name: fixture.fields.radio.label })
  const radioField = fixture.fields.radio
  const checked =
    version === "next"
      ? radio.getByRole("radio", { name: radioField.nextLabel, exact: true })
      : radio.locator('input[type="radio"]:checked')
  await expect(checked).toBeChecked()
  await expect(checked).toHaveValue(String(radioField[version]))
  const selectField = fixture.fields.select
  const select = editor.getByLabel(selectField.label, { exact: true })
  await expect(select).toHaveValue(String(selectField[version]))
}

/** @param {import("@playwright/test").Page} page @param {Fixture} fixture */
async function openEditor(page, fixture) {
  await page.goto(`${preview}/${fixture.slug}`, { waitUntil: "networkidle" })
  await expect(page.locator("#edit-button")).toBeVisible()
  await page.locator("#edit-button").click()
  await expect(page.locator("#editor")).toBeVisible()
}

test("authenticated data-form edit persists typed values without dropping untouched values", async () => {
  const previewPasswordFile = process.env.COBALT_LOCAL_PASSWORD_FILE
  const adminPasswordFile = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  const fixtureFile = process.env.COBALT_FORMS_FIXTURE
  assert.ok(
    previewPasswordFile && adminPasswordFile && fixtureFile,
    "explicit preview, admin, and prepared sacrificial fixture paths required"
  )
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(fixtureFile, "utf8"))
  assert.equal(
    fixture.sacrificial,
    true,
    "only a prepared sacrificial fixture may be edited"
  )
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(
    fixture.username,
    "cobalt-import",
    "only the synthetic fixture account may edit"
  )
  assert.equal(fixture.siteId, 6000000)
  assert.equal(fixture.slug, "local-acceptance:form-roundtrip")
  assert.deepEqual(Object.keys(fixture.fields).sort(), [
    "radio",
    "select",
    "text",
    "wiki"
  ])
  assert.ok(fixture.unchangedKeys.length && fixture.unknownKeys.length)
  for (const key of [...fixture.unchangedKeys, ...fixture.unknownKeys]) {
    assert.ok(
      Object.hasOwn(fixture.initialValues, key) &&
        Object.hasOwn(fixture.expectedValues, key)
    )
    assert.equal(digest(fixture.initialValues[key]), digest(fixture.expectedValues[key]))
  }
  for (const field of Object.values(fixture.fields)) {
    assert.equal(digest(fixture.initialValues[field.name]), digest(field.initial))
    assert.equal(digest(fixture.expectedValues[field.name]), digest(field.next))
  }

  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      httpCredentials: {
        username: "cobalt",
        password: (await readFile(previewPasswordFile, "utf8")).trim(),
        origin: preview
      }
    })
    const page = await context.newPage()
    await page.goto(`${preview}/-/login`, { waitUntil: "networkidle" })
    await expect(page.locator("#login")).toBeVisible()
    await page.locator('#login [name="nameOrEmail"]').fill(fixture.username)
    await page
      .locator('#login [name="password"]')
      .fill((await readFile(adminPasswordFile, "utf8")).trim())
    await page.locator("#login button[type=submit]").click()
    await expect(page.locator("#login")).toHaveCount(0)
    const sessionCookie = (await context.cookies()).find(
      (cookie) => cookie.name === "wikijump_token" && cookie.domain === "127.0.0.1"
    )
    assert.ok(sessionCookie, "login must create a real browser session")
    const sessionToken = decodeURIComponent(sessionCookie.value)

    const initial = await readPage(context.request, fixture, sessionToken)
    assertValues(initial.form.values, fixture.initialValues, "initial")
    const fieldNames = initial.form.schema.fields.map((field) => field.name)
    for (const key of fixture.unknownKeys)
      assert.ok(!fieldNames.includes(key), "unknown key must be outside schema")
    for (const field of Object.values(fixture.fields))
      assert.ok(fieldNames.includes(field.name))
    const initialRevision = initial.page_revision.revision_id
    assert.ok(initialRevision, "fixture revision required")

    const anonymousContext = await browser.newContext({
      httpCredentials: {
        username: "cobalt",
        password: (await readFile(previewPasswordFile, "utf8")).trim(),
        origin: preview
      }
    })
    try {
      const anonymous = await anonymousContext.newPage()
      await anonymous.goto(`${preview}/${fixture.slug}`, { waitUntil: "networkidle" })
      await anonymous.locator("#edit-button").click()
      await expect(anonymous.locator("#editor")).toHaveCount(0)
      const denied = await anonymousContext.request.post(
        `${preview}/${fixture.slug}?/edit`,
        {
          headers: { accept: "application/json", "x-sveltekit-action": "true" },
          form: {
            pageId: String(initial.page.page_id),
            siteId: String(fixture.siteId),
            lastRevisionId: String(initialRevision),
            title: initial.page_revision.title,
            altTitle: initial.page_revision.alt_title ?? "",
            tags: (initial.page_revision.tags ?? []).join(" "),
            comments: "",
            wikitext: JSON.stringify({
              ...fixture.initialValues,
              [fixture.fields.text.name]: "Unauthorized fixture change"
            })
          }
        }
      )
      const deniedAction = await denied.json()
      assert.equal(
        deniedAction.type,
        "failure",
        "anonymous edit action must reject writes"
      )
      assert.ok(deniedAction.status >= 400)
      const unchanged = await readPage(anonymousContext.request, fixture, null)
      assert.equal(unchanged.page_revision.revision_id, initialRevision)
      assertValues(
        unchanged.form.values,
        fixture.initialValues,
        "after unauthorized action"
      )

      await openEditor(page, fixture)
      await assertControls(page, fixture, "initial")

      const editor = page.locator("#editor")
      await editor
        .getByLabel(fixture.fields.text.label, { exact: true })
        .fill(String(fixture.fields.text.next))
      await editor
        .getByLabel(fixture.fields.wiki.label, { exact: true })
        .fill(String(fixture.fields.wiki.next))
      await editor
        .getByRole("group", { name: fixture.fields.radio.label })
        .getByRole("radio", { name: fixture.fields.radio.nextLabel, exact: true })
        .check()
      await editor.getByLabel(fixture.fields.select.label, { exact: true }).selectOption({
        label: fixture.fields.select.nextLabel
      })
      const savedResponse = page.waitForResponse(
        (response) =>
          response.request().method() === "POST" && response.url().includes("?/edit")
      )
      await editor.locator('[type="submit"]').click()
      assert.equal((await savedResponse).status(), 200, "authorized save must succeed")
      await expect(editor).toHaveCount(0)
      const saved = await readPage(context.request, fixture, sessionToken)
      assert.notEqual(
        saved.page_revision.revision_id,
        initialRevision,
        "save must create a revision"
      )
      assertValues(saved.form.values, fixture.expectedValues, "saved")
      for (const key of [...fixture.unchangedKeys, ...fixture.unknownKeys]) {
        assert.equal(
          digest(saved.form.values[key]),
          digest(initial.form.values[key]),
          `${key} survived edit`
        )
      }

      await page.reload({ waitUntil: "networkidle" })
      await openEditor(page, fixture)
      await assertControls(page, fixture, "next")
    } finally {
      await anonymousContext.close()
    }
  } finally {
    await browser.close()
  }
})
