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
  assert.equal(kinds.static?.length, 12, "static field count")
  assert.equal(kinds.text?.length, 8, "text field count")
  assert.equal(kinds.select?.length, 7, "select field count")
  assert.equal(kinds.wiki?.length, 1, "wiki field count")
  assert.equal(new Set(fields.map((field) => field.name)).size, 28, "unique fields")
  for (const label of ["Summary", "Additional Content Warnings", "Digest?"]) {
    assert.ok(
      fields.some((field) => field.kind === "static" && field.properties.label === label),
      `${label} must be an archived static-field label`
    )
  }
  for (const name of [
    "summary",
    "additionalCW",
    "digest",
    "characters",
    "references",
    "content"
  ]) {
    const field = fields.find((candidate) => candidate.name === name)
    assert.ok(field, `${name} archived field required`)
    assert.notEqual(field.kind, "static", `${name} editable field required`)
    assert.equal(field.properties.label, null, `${name} archived null label`)
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
 * @param {unknown} value
 * @param {string} visibleLabel
 */
async function assertRadioControl(wrapper, field, value, visibleLabel) {
  const legend = wrapper.locator("legend")
  await expect(legend).toHaveCount(visibleLabel ? 1 : 0)
  if (visibleLabel) await expect(legend).toHaveText(visibleLabel)
  await expect(wrapper.locator("fieldset")).toHaveAccessibleName(
    visibleLabel || field.name
  )
  const radios = await wrapper.locator('input[type="radio"]').evaluateAll((inputs) =>
    inputs.map((input) => {
      if (!(input instanceof HTMLInputElement)) {
        throw new TypeError("Expected radio input")
      }
      return {
        value: input.value,
        label: input.closest("label")?.textContent?.trim(),
        checked: input.checked
      }
    })
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
}

/**
 * @param {import("@playwright/test").Locator} control
 * @param {import("../../src/lib/form-editor").FormField} field
 * @param {unknown} value
 */
async function assertSelectControl(control, field, value) {
  const options = await control.locator("option").evaluateAll((nodes) =>
    nodes.map((node) => {
      if (!(node instanceof HTMLOptionElement)) {
        throw new TypeError("Expected option")
      }
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
    `${field.name} options digest`
  )
  assert.equal(
    digest(await control.inputValue()),
    digest(text(value)),
    `${field.name} default digest`
  )
}

/** @param {import("../../src/lib/form-editor").FormField} field */
function controlSelector(field) {
  if (
    field.kind === "wiki" ||
    (field.kind === "text" && Number(field.properties.height) >= 2)
  ) {
    return "textarea"
  }
  return field.kind === "select" ? "select" : 'input[type="text"]'
}

/**
 * @param {import("@playwright/test").Locator} wrapper
 * @param {import("../../src/lib/form-editor").FormField} field
 * @param {import("@playwright/test").Locator} control
 */
async function assertAfterGuidance(wrapper, field, control) {
  const after = text(field.properties.after)
  const help = wrapper.locator(".field-control > small")
  await expect(help).toHaveCount(after ? 1 : 0)
  if (after) {
    await expect(help).toHaveText(after)
    await expect(help.locator("*")).toHaveCount(0)
    const id = await help.getAttribute("id")
    assert.ok(id, `${field.name} help id required`)
    await expect(control).toHaveAttribute("aria-describedby", id)
    await expect(wrapper.page().locator(`[id="${id}"]`)).toHaveCount(1)
  } else {
    await expect(control).not.toHaveAttribute("aria-describedby")
  }
}

/**
 * @param {import("../../src/lib/form-editor").FormField} field
 * @param {import("@playwright/test").Locator} control
 */
async function assertPlaceholder(field, control) {
  if (field.kind === "text" || field.kind === "wiki") {
    const hint = text(field.properties.hint)
    if (hint) await expect(control).toHaveAttribute("placeholder", hint)
    else await expect(control).not.toHaveAttribute("placeholder")
  }
}

/**
 * @param {import("../../src/lib/form-editor").FormField} field
 * @param {import("@playwright/test").Locator} control
 */
async function assertDimensions(field, control) {
  const width = Number(field.properties.width)
  if (Number.isInteger(width) && width > 0) {
    await expect(control).toHaveAttribute(
      controlSelector(field) === "textarea" ? "cols" : "size",
      String(width)
    )
  }
  const height = Number(field.properties.height)
  if (controlSelector(field) === "textarea" && Number.isInteger(height) && height > 0) {
    await expect(control).toHaveAttribute("rows", String(height))
  }
}

/**
 * @param {import("@playwright/test").Locator} wrapper
 * @param {import("../../src/lib/form-editor").FormField} field
 * @param {import("@playwright/test").Locator} control
 */
async function assertGuidance(wrapper, field, control) {
  await assertAfterGuidance(wrapper, field, control)
  await assertPlaceholder(field, control)
}

/**
 * @param {import("@playwright/test").Locator} wrapper
 * @param {import("../../src/lib/form-editor").FormField} field
 */
async function assertDefaultControl(wrapper, field) {
  const visibleLabel = text(field.properties.label)
  const accessibleName = visibleLabel || field.name
  const value = field.properties.default
  if (field.kind === "static") {
    await expect(wrapper.locator(".static-label")).toHaveCount(visibleLabel ? 1 : 0)
    if (visibleLabel) await assertStaticLabel(wrapper, visibleLabel)
    assert.equal(
      digest(await wrapper.locator(".static-field").textContent()),
      digest(text(field.properties.value ?? value)),
      `${field.name} static content digest`
    )
    return
  }
  if (field.kind === "select" && field.options.length >= 2 && field.options.length <= 4) {
    await assertRadioControl(wrapper, field, value, visibleLabel)
    await assertGuidance(wrapper, field, wrapper.locator("fieldset"))
    return
  }
  const label = wrapper.locator("label")
  await expect(label).toHaveCount(visibleLabel ? 1 : 0)
  if (visibleLabel) await expect(label).toHaveText(visibleLabel)
  const control = wrapper.locator(controlSelector(field))
  await expect(control).toHaveCount(1)
  await expect(control).toHaveAccessibleName(accessibleName)
  if (field.kind === "select") await assertSelectControl(control, field, value)
  else {
    assert.equal(
      digest(await control.inputValue()),
      digest(text(value)),
      `${field.name} default digest`
    )
    await assertDimensions(field, control)
  }
  await assertGuidance(wrapper, field, control)
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

/**
 * @param {import("@playwright/test").Locator} locator
 * @param {string} name
 */
async function visibleBox(locator, name) {
  await expect(locator).toBeVisible()
  const box = await locator.boundingBox()
  assert.ok(box, `${name} visible rectangle required`)
  return box
}

/** @param {number} actual @param {number} expected @param {string} name */
function assertNear(actual, expected, name) {
  assert.ok(
    Math.abs(actual - expected) <= 2,
    `${name}: expected ${expected} ± 2, got ${actual}`
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("../../src/lib/form-editor").FormField[]} fields
 */
async function assertWritingLayout(page, fields) {
  const rows = page.locator("#editor .form-field")
  /** @param {string} name */
  const row = (name) => {
    const index = fields.findIndex((field) => field.name === name)
    assert.ok(index >= 0, `${name} writing field required`)
    return rows.nth(index)
  }
  const authorLabel = await visibleBox(row("author").locator("label"), "Author label")
  const authorInput = await visibleBox(
    row("author").locator('input[type="text"]'),
    "Author input"
  )
  const imageLabel = await visibleBox(row("image").locator("label"), "Image label")
  const imageInput = await visibleBox(
    row("image").locator('input[type="text"]'),
    "Image input"
  )
  for (const [name, label, control] of [
    ["Author", authorLabel, authorInput],
    ["Image", imageLabel, imageInput]
  ]) {
    assertNear(control.x - label.x, 106, `${name} label-to-control offset`)
    assert.ok(
      label.y < control.y + control.height,
      `${name} label overlaps control vertically`
    )
    assert.ok(
      control.y < label.y + label.height,
      `${name} control overlaps label vertically`
    )
  }

  const summary = await visibleBox(row("summary").locator("textarea"), "Summary")
  const content = await visibleBox(row("content").locator("textarea"), "Content")
  for (const [name, control] of [
    ["Image", imageInput],
    ["Summary", summary],
    ["Content", content]
  ]) {
    assertNear(control.x, authorInput.x, `${name} control-column alignment`)
  }

  const formatLabel = await visibleBox(row("format").locator("legend"), "Format label")
  assertNear(formatLabel.x, authorLabel.x, "Format label-column alignment")
  const radios = await row("format").locator('input[type="radio"]').all()
  assert.equal(radios.length, 4, "Format has four visible radio options")
  const radioBoxes = await Promise.all(
    radios.map((radio, index) => visibleBox(radio, `Format radio ${index + 1}`))
  )
  for (const [index, radio] of radioBoxes.entries()) {
    assertNear(radio.y, radioBoxes[0].y, `Format radio ${index + 1} horizontal alignment`)
    if (index > 0) {
      assert.ok(radio.x > radioBoxes[index - 1].x, "Format radios advance left to right")
    }
  }
  assert.ok(formatLabel.y < radioBoxes[0].y + radioBoxes[0].height)
  assert.ok(radioBoxes[0].y < formatLabel.y + formatLabel.height)
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
    const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } })
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
      assert.ok(existing.type === "found", "real writing page required")
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
      await assertWritingLayout(page, fields)
      const author = fields.find((field) => field.name === "author")
      const summary = fields.find((field) => field.name === "summary")
      assert.ok(author && summary, "archived author and summary required")
      await expect(
        page
          .locator("#editor .form-field")
          .nth(fields.indexOf(author))
          .locator('input[type="text"]')
      ).toHaveAttribute("placeholder", "Your player name")
      const summaryControl = page
        .locator("#editor .form-field")
        .nth(fields.indexOf(summary))
        .locator("textarea")
      await expect(summaryControl).toHaveAttribute("rows", "3")
      await expect(summaryControl).toHaveAttribute("cols", "80")
      for (const [name, after] of Object.entries({
        image: 'Optional "cover" image. Leave blank for no image.',
        additionalCW: 'Leave as "@@" if no additional content warnings',
        arc: 'Leave as "@@" if no Arc',
        chain: 'Leave as "@@" if no Chain'
      })) {
        const field = fields.find((candidate) => candidate.name === name)
        assert.ok(field, `${name} archived field required`)
        assert.equal(field.properties.after, after, `${name} archived help`)
        await assertGuidance(
          page.locator("#editor .form-field").nth(fields.indexOf(field)),
          field,
          page
            .locator("#editor .form-field")
            .nth(fields.indexOf(field))
            .locator(controlSelector(field))
        )
      }

      const wiki = fields.find((field) => field.kind === "wiki")
      assert.ok(wiki, "archived writing wiki field required")
      const marker = `Local writing preview ${randomBytes(8).toString("hex")}`
      const wikiControl = page
        .locator("#editor .form-field")
        .nth(fields.indexOf(wiki))
        .locator("textarea")
      const summaryMarker = `Summary line one ${randomBytes(6).toString("hex")}\nSummary line two`
      await summaryControl.fill(summaryMarker)
      await wikiControl.fill(marker)
      const previewResponse = page.waitForResponse(
        (response) =>
          response.request().method() === "POST" && response.url().endsWith("?/preview")
      )
      await page.locator("#edit-preview-button").click()
      const response = await previewResponse
      assert.equal(response.status(), 200, "writing preview HTTP status")
      const previewBody = await response.text()
      assert.ok(previewBody.includes(marker), "preview response contains wiki marker")
      const region = page.locator('section[aria-label="Page preview"]')
      await expect(region).toHaveAttribute("aria-busy", "false")
      await expect(region).toContainText(marker)
      await expect(summaryControl).toHaveValue(summaryMarker)
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
