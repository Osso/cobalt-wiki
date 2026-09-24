import assert from "node:assert/strict"
import { createHash, randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const preview = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000
const existingSlug = "writing:2024-03-13-atley-in-a-nutshell"
const templateSlug = "writing:_template"

/** @param {unknown} value */
const digest = (value) => createHash("sha256").update(JSON.stringify(value)).digest("hex")

/** @param {unknown} value */
const text = (value) =>
  typeof value === "string" || typeof value === "number" || typeof value === "boolean"
    ? String(value)
    : ""

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} method
 * @param {object} params
 */
async function rpc(request, method, params) {
  const response = await request.post(backend, {
    data: { jsonrpc: "2.0", id: 1, method, params }
  })
  assert.equal(response.status(), 200, `${method} HTTP status`)
  const payload = await response.json()
  assert.ok(!payload.error, `${method} RPC error code ${payload.error?.code ?? "?"}`)
  return payload.result
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} slug
 * @param {string} token
 */
async function readView(request, slug, token) {
  return rpc(request, "page_view", {
    site_id: siteId,
    locales: ["en"],
    session_token: token,
    route: { slug, extra: "" }
  })
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} slug
 */
async function readStored(request, slug) {
  const stored = await rpc(request, "page_get", {
    site_id: siteId,
    page: slug,
    details: { wikitext: true }
  })
  assert.ok(stored, `${slug} must remain stored`)
  assert.ok(Number.isSafeInteger(stored.revision_id), `${slug} revision required`)
  assert.equal(typeof stored.wikitext, "string", `${slug} source required`)
  return { revision: stored.revision_id, source: digest(stored.wikitext) }
}

/** @param {import("../../src/lib/form-editor").FormField[]} fields */
function assertWritingSchema(fields) {
  assert.equal(fields.length, 28, "archived writing field count")
  const kinds = Object.groupBy(fields, (field) => field.kind)
  for (const [kind, count] of Object.entries({
    static: 12,
    text: 8,
    select: 7,
    wiki: 1
  })) {
    assert.equal(kinds[kind]?.length, count, `${kind} field count`)
  }
  assert.equal(new Set(fields.map((field) => field.name)).size, 28, "unique fields")
  for (const label of ["Summary", "Additional Content Warnings", "Digest?"]) {
    assert.ok(
      fields.some((field) => field.kind === "static" && field.properties.label === label),
      `${label} must be an archived static-field label`
    )
  }
}

/**
 * @param {import("@playwright/test").Locator} wrapper
 * @param {string} label
 */
async function assertStaticLabel(wrapper, label) {
  const visibleLabel = wrapper.locator(".static-label")
  await expect(visibleLabel).toBeVisible()
  await expect(visibleLabel).toHaveText(label)
}

/**
 * @param {import("@playwright/test").Locator} wrapper
 * @param {import("../../src/lib/form-editor").FormField} field
 */
async function assertDefaultControl(wrapper, field) {
  const label = text(field.properties.label) || field.name
  const value = field.properties.default
  if (field.kind === "static") {
    if (text(field.properties.label)) await assertStaticLabel(wrapper, label)
    assert.equal(
      digest(await wrapper.locator(".static-field").textContent()),
      digest(text(field.properties.value ?? value)),
      `${field.name} static content digest`
    )
    return
  }
  if (field.kind === "select" && field.options.length >= 2 && field.options.length <= 4) {
    await expect(wrapper.locator("legend")).toHaveText(label)
    const radios = await wrapper.locator('input[type="radio"]').evaluateAll((inputs) =>
      inputs.map((input) => ({
        value: input.value,
        label: input.closest("label")?.textContent?.trim(),
        checked: input.checked
      }))
    )
    assert.equal(
      digest(
        radios.map(({ value: code, label: optionLabel }) => ({
          code,
          label: optionLabel
        }))
      ),
      digest(
        field.options.map((option) => ({
          code: text(option.code),
          label: text(option.label)
        }))
      ),
      `${field.name} radio options digest`
    )
    assert.equal(
      digest(radios.filter((radio) => radio.checked).map((radio) => radio.value)),
      digest(field.options.some((option) => option.code === value) ? [text(value)] : []),
      `${field.name} selected radio digest`
    )
    return
  }
  await expect(wrapper.locator("label").first()).toHaveText(label)
  const control = wrapper.locator("input, textarea, select").first()
  if (field.kind === "select") {
    const options = await control
      .locator("option")
      .evaluateAll((nodes) =>
        nodes.map((node) => ({ code: node.value, label: node.textContent?.trim() }))
      )
    const declared = field.options.map((option) => ({
      code: text(option.code),
      label: text(option.label)
    }))
    const unknown = !field.options.some((option) => option.code === value)
    assert.equal(
      digest(options),
      digest(
        unknown ? [{ code: text(value), label: text(value) }, ...declared] : declared
      ),
      `${field.name} options digest`
    )
  }
  assert.equal(
    digest(await control.inputValue()),
    digest(text(value)),
    `${field.name} default digest`
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("../../src/lib/form-editor").FormField[]} fields
 */
async function assertNewForm(page, fields) {
  const editor = page.locator("#editor")
  await expect(editor).toBeVisible()
  await expect(editor.locator('textarea[name="wikitext"], .editor-wikitext')).toHaveCount(
    0
  )
  const rows = editor.locator(".form-field")
  await expect(rows).toHaveCount(28)
  for (const [index, field] of fields.entries()) {
    await assertDefaultControl(rows.nth(index), field)
  }
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {string} slug
 * @param {Map<string, { revision: number; source: string }>} baseline
 */
async function assertNoWrites(request, token, slug, baseline) {
  assert.equal(
    (await readView(request, slug, token))?.type,
    "missing",
    "target must stay missing"
  )
  for (const [storedSlug, before] of baseline) {
    assert.deepEqual(
      await readStored(request, storedSlug),
      before,
      `${storedSlug} unchanged`
    )
  }
}

test("NewPage writing form labels, defaults and preview do not write", async () => {
  const adminPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(adminPath, "cobalt-import password file required")
  const suffix = randomBytes(8).toString("hex")
  const title = `(2000-01-01) Local writing form preview ${suffix}`
  const slug = `writing:2000-01-01-local-writing-form-preview-${suffix}`
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext()
    try {
      const page = await context.newPage()
      await page.goto(`${preview}/-/login`, { waitUntil: "networkidle" })
      await expect(page.locator("#login")).toBeVisible()
      await page.locator('#login [name="nameOrEmail"]').fill("cobalt-import")
      await page
        .locator('#login [name="password"]')
        .fill((await readFile(adminPath, "utf8")).trim())
      await page.locator("#login button[type=submit]").click()
      await expect(page.locator("#login")).toHaveCount(0)
      const cookie = (await context.cookies()).find(
        (entry) => entry.name === "wikijump_token" && entry.domain === "127.0.0.1"
      )
      assert.ok(cookie, "authenticated browser session required")
      const token = decodeURIComponent(cookie.value)
      const request = context.request
      const existing = await readView(request, existingSlug, token)
      assert.equal(existing?.type, "found", "real writing page required")
      assert.ok(existing.data.form, "real writing form required")
      const fields = existing.data.form.schema.fields
      assertWritingSchema(fields)
      assert.ok(
        fields.some((field) => field.kind !== "static" && text(field.properties.default)),
        "archived writing form must include a nonempty editable default"
      )
      assert.equal(
        (await readView(request, templateSlug, token))?.type,
        "found",
        "template required"
      )
      const baseline = new Map([
        [existingSlug, await readStored(request, existingSlug)],
        [templateSlug, await readStored(request, templateSlug)]
      ])
      await assertNoWrites(request, token, slug, baseline)

      await page.goto(`${preview}/new-writing`, { waitUntil: "networkidle" })
      const newPage = page.locator(
        '.new-page-box form:has(input[value="Create Writing"])'
      )
      await expect(newPage).toBeVisible()
      await newPage.locator('[name="pageName"]').fill(title)
      await newPage.locator('[type="submit"]').click()
      const expectedPath = `/${slug}/edit/true/title/${encodeURIComponent(title)}`
      await page.waitForURL((url) => url.pathname === expectedPath, {
        waitUntil: "networkidle"
      })
      await expect(page.locator('#editor [name="title"]')).toHaveValue(title)
      await assertNewForm(page, fields)

      const wiki = fields.find((field) => field.kind === "wiki")
      assert.ok(wiki, "archived writing wiki field required")
      const marker = `Local writing preview ${randomBytes(8).toString("hex")}`
      const wikiControl = page
        .locator("#editor .form-field")
        .nth(fields.indexOf(wiki))
        .locator("textarea")
      await wikiControl.fill(marker)
      const previewResponse = page.waitForResponse(
        (response) =>
          response.request().method() === "POST" && response.url().endsWith("?/preview")
      )
      await page.locator("#edit-preview-button").click()
      assert.equal((await previewResponse).status(), 200, "writing preview HTTP status")
      const region = page.locator('section[aria-label="Page preview"]')
      await expect(region).toHaveAttribute("aria-busy", "false")
      await expect(region).toContainText(marker)
      await assertNoWrites(request, token, slug, baseline)

      await page.reload({ waitUntil: "networkidle" })
      await expect(page.locator('#editor [name="title"]')).toHaveValue(title)
      await assertNewForm(page, fields)
      await assertNoWrites(request, token, slug, baseline)
    } finally {
      await context.close()
    }
  } finally {
    await browser.close()
  }
})
