import assert from "node:assert/strict"
import { createHash, randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const preview = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000
const existingSlug = "character:atley"
const templateSlug = "character:_template"

/** @param {unknown} value */
const digest = (value) => createHash("sha256").update(JSON.stringify(value)).digest("hex")

/** @param {unknown} value */
const text = (value) =>
  typeof value === "string" || typeof value === "number" || typeof value === "boolean"
    ? String(value)
    : ""

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} slug @param {string} token
 */
async function readPage(request, slug, token) {
  const response = await request.post(backend, {
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "page_view",
      params: {
        site_id: siteId,
        locales: ["en"],
        session_token: token,
        route: { slug, extra: "" }
      }
    }
  })
  assert.equal(response.status(), 200, `page_view status for ${slug}`)
  /**
   * @type {{
   *   error?: { code?: string }
   *   result?: import("../../src/lib/server/deepwell/views").PageView
   * }}
   */
  const payload = await response.json()
  assert.ok(!payload.error, `page_view error code ${payload.error?.code ?? "?"}`)
  assert.ok(payload.result, `page_view result required for ${slug}`)
  return payload.result
}

/**
 * @param {import("../../src/lib/form-editor").FormField} field @param
 *   {import("../../src/lib/form-editor").FormValues} values
 */
function initialValue(field, values) {
  if (Object.hasOwn(values, field.name)) return values[field.name]
  return field.properties.default
}

/** @param {import("../../src/lib/form-editor").FormField[]} fields */
function assertSchema(fields) {
  assert.equal(fields.length, 33, "archived character form field count")
  const counts = Object.groupBy(fields, (field) => field.kind)
  assert.equal(counts.static?.length, 11, "static field count")
  assert.equal(counts.text?.length, 10, "text field count")
  assert.equal(counts.select?.length, 7, "select field count")
  assert.equal(counts.wiki?.length, 5, "wiki field count")
  assert.equal(
    new Set(fields.map((field) => field.name)).size,
    33,
    "unique ordered fields"
  )
  assert.equal(fields.find((field) => field.name === "name")?.properties.label, "Name")
  assert.equal(fields.find((field) => field.name === "name")?.kind, "text")
}

/**
 * @param {import("@playwright/test").Locator} wrapper
 * @param {import("../../src/lib/form-editor").FormField} field
 * @param {unknown} value
 */
async function assertStaticField(wrapper, field, value) {
  const expected = text(field.properties.value ?? value)
  const visible = await wrapper.locator(".static-field").textContent()
  assert.equal(digest(visible), digest(expected), `static content digest: ${field.name}`)
}

/**
 * @param {import("@playwright/test").Locator} wrapper
 * @param {import("../../src/lib/form-editor").FormField} field
 * @param {unknown} value
 * @param {string} label
 */
async function assertRadioField(wrapper, field, value, label) {
  await expect(wrapper.locator("legend")).toHaveText(label)
  const radios = await wrapper.locator('input[type="radio"]').evaluateAll((inputs) =>
    inputs.map((input) => {
      if (!(input instanceof HTMLInputElement)) {
        throw new TypeError("Expected radio input")
      }
      return {
        code: input.value,
        label: input.closest("label")?.textContent?.trim(),
        checked: input.checked
      }
    })
  )
  const declared = field.options.map((option) => ({
    code: text(option.code),
    label: text(option.label)
  }))
  assert.equal(
    digest(radios.map(({ code, label: optionLabel }) => ({ code, label: optionLabel }))),
    digest(declared),
    `${field.name} radio option digest in source order`
  )
  assert.equal(
    digest(radios.filter((radio) => radio.checked).map((radio) => radio.code)),
    digest(field.options.some((option) => option.code === value) ? [text(value)] : []),
    `${field.name} selected option digest`
  )
}

/**
 * @param {import("@playwright/test").Locator} control
 * @param {import("../../src/lib/form-editor").FormField} field
 * @param {unknown} value
 */
async function assertSelectField(control, field, value) {
  const options = await control.locator("option").evaluateAll((nodes) =>
    nodes.map((node) => {
      if (!(node instanceof HTMLOptionElement)) throw new TypeError("Expected option")
      return { code: node.value, label: node.textContent?.trim() }
    })
  )
  const declared = field.options.map((option) => ({
    code: text(option.code),
    label: text(option.label)
  }))
  const unknown = !field.options.some((option) => option.code === value)
  assert.equal(
    digest(options),
    digest(unknown ? [{ code: text(value), label: text(value) }, ...declared] : declared),
    `${field.name} select option digest in source order`
  )
  assert.equal(
    digest(await control.inputValue()),
    digest(text(value)),
    `${field.name} selected value digest`
  )
}

/**
 * @param {import("@playwright/test").Locator} control
 * @param {import("../../src/lib/form-editor").FormField} field
 */
async function assertDimensions(control, field) {
  const width = Number(field.properties.width)
  if (Number.isInteger(width) && width > 0) {
    await expect(control).toHaveAttribute(
      field.kind === "wiki" ? "cols" : "size",
      String(width)
    )
  }
  if (field.kind === "wiki") {
    const height = Number(field.properties.height)
    if (Number.isInteger(height) && height > 0) {
      await expect(control).toHaveAttribute("rows", String(height))
    }
  }
}

/**
 * @param {import("@playwright/test").Locator} wrapper
 * @param {import("../../src/lib/form-editor").FormField} field
 * @param {import("../../src/lib/form-editor").FormValues} values
 */
async function assertFieldControl(wrapper, field, values) {
  const label = text(field.properties.label) || field.name
  const value = initialValue(field, values)
  if (field.kind === "static") return assertStaticField(wrapper, field, value)
  if (field.kind === "select" && field.options.length >= 2 && field.options.length <= 4) {
    return assertRadioField(wrapper, field, value, label)
  }
  await expect(wrapper.locator("label").first()).toHaveText(label)
  const control = wrapper.locator("input, textarea, select").first()
  if (field.kind === "select") return assertSelectField(control, field, value)
  assert.equal(
    digest(await control.inputValue()),
    digest(text(value)),
    `${field.name} value digest`
  )
  await assertDimensions(control, field)
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("../../src/lib/form-editor").PageForm} form
 */
async function assertFormControls(page, form) {
  const editor = page.locator("#editor")
  await expect(editor).toBeVisible()
  await expect(editor.locator('textarea[name="wikitext"], .editor-wikitext')).toHaveCount(
    0
  )
  const controls = editor.locator(".form-field")
  await expect(controls).toHaveCount(form.schema.fields.length)
  for (const [index, field] of form.schema.fields.entries()) {
    await assertFieldControl(controls.nth(index), field, form.values)
  }
}

/**
 * @param {import("@playwright/test").Page} page @param {string} slug
 * @param {string} name
 */
async function previewName(page, slug, name) {
  await page.locator("#editor").getByLabel("Name", { exact: true }).fill(name)
  const responsePromise = page.waitForResponse(
    (response) =>
      response.request().method() === "POST" &&
      response.url().includes(`/${slug}/edit`) &&
      response.url().endsWith("?/preview")
  )
  await page.locator("#edit-preview-button").click()
  const response = await responsePromise
  assert.equal(response.status(), 200, `${slug} preview status`)
  const region = page.locator('section[aria-label="Page preview"]')
  await expect(region).toHaveAttribute("aria-busy", "false")
  assert.ok(
    await region.evaluate(
      (element, marker) => element.textContent?.includes(marker),
      name
    ),
    "preview must render edited Name"
  )
  await expect(page.locator("#editor").getByLabel("Name", { exact: true })).toHaveValue(
    name
  )
}

test("imported character form renders and previews without persisting either page", async () => {
  const gatewayPath = process.env.COBALT_LOCAL_PASSWORD_FILE
  const adminPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(
    gatewayPath && adminPath,
    "protected gateway and cobalt-import password files required"
  )
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      httpCredentials: {
        username: "cobalt",
        password: (await readFile(gatewayPath, "utf8")).trim(),
        origin: preview
      }
    })
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
      const existing = await readPage(context.request, existingSlug, token)
      assert.ok(existing.type === "found", "imported character required")
      assert.ok(existing.data.form, "existing character must expose a form")
      assert.equal(typeof existing.data.wikitext, "string", "existing source required")
      const form = existing.data.form
      assertSchema(form.schema.fields)
      assert.ok(existing.data.page_revision.revision_id, "existing revision required")
      const template = await readPage(context.request, templateSlug, token)
      assert.ok(template.type === "found", "character template required")
      assert.ok(!template.data.form, "template must remain raw source")

      await page.goto(`${preview}/${existingSlug}/edit`, { waitUntil: "networkidle" })
      await assertFormControls(page, form)
      const marker = `Character preview ${randomBytes(6).toString("hex")}`
      await previewName(page, existingSlug, marker)
      const after = await readPage(context.request, existingSlug, token)
      assert.ok(after.type === "found", "existing page must remain present")
      assert.ok(after.data.form, "existing form must remain present")
      assert.equal(
        typeof after.data.wikitext,
        "string",
        "existing source must remain present"
      )
      assert.equal(
        after.data.page_revision.revision_id,
        existing.data.page_revision.revision_id
      )
      assert.equal(
        digest(after.data.wikitext),
        digest(existing.data.wikitext),
        "source digest"
      )
      assert.equal(
        digest(after.data.form.values),
        digest(form.values),
        "stored values digest"
      )
      await page.reload({ waitUntil: "networkidle" })
      await assertFormControls(page, form)
      const reloaded = await readPage(context.request, existingSlug, token)
      assert.ok(reloaded.type === "found", "reloaded character must remain present")
      assert.ok(reloaded.data.form, "reloaded form must remain present")
      assert.equal(typeof reloaded.data.wikitext, "string", "reloaded source required")
      assert.equal(
        reloaded.data.page_revision.revision_id,
        existing.data.page_revision.revision_id
      )
      assert.equal(
        digest(reloaded.data.wikitext),
        digest(existing.data.wikitext),
        "reloaded source digest"
      )
      assert.equal(
        digest(reloaded.data.form.values),
        digest(form.values),
        "reloaded values digest"
      )

      const slug = `character:local-form-preview-${randomBytes(8).toString("hex")}`
      const title = `Local form preview ${randomBytes(4).toString("hex")}`
      assert.equal((await readPage(context.request, slug, token))?.type, "missing")
      await page.goto(`${preview}/${slug}/edit/true/title/${encodeURIComponent(title)}`, {
        waitUntil: "networkidle"
      })
      await expect(page.locator('#editor [name="title"]')).toHaveValue(title)
      const defaults = { schema: form.schema, values: {} }
      assert.equal(
        defaults.schema.fields.find((field) => field.name === "aka")?.properties.default,
        "@@",
        "archived AKA default"
      )
      await assertFormControls(page, defaults)
      await previewName(page, slug, marker)
      assert.equal(
        (await readPage(context.request, slug, token))?.type,
        "missing",
        "preview must not create a page"
      )
      await page.reload({ waitUntil: "networkidle" })
      await assertFormControls(page, defaults)
      assert.equal(
        (await readPage(context.request, slug, token))?.type,
        "missing",
        "reload must not create a page"
      )
    } finally {
      await context.close()
    }
  } finally {
    await browser.close()
  }
})
