import assert from "node:assert/strict"
import { createHash, randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const preview = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000
const categories = [
  {
    name: "player",
    markerField: "pronouns",
    counts: { static: 2, text: 6, wiki: 3, select: 0 }
  },
  {
    name: "chain",
    markerField: "summary",
    counts: { static: 2, text: 3, wiki: 0, select: 0 }
  },
  {
    name: "arc",
    markerField: "arcSummary",
    counts: { static: 4, text: 5, wiki: 1, select: 0 }
  },
  {
    name: "bgc",
    markerField: "name",
    counts: { static: 8, text: 9, wiki: 2, select: 5 }
  },
  {
    name: "npc",
    markerField: "name",
    counts: { static: 7, text: 9, wiki: 2, select: 4 }
  },
  {
    name: "application",
    markerField: "pronouns",
    counts: { static: 23, text: 0, wiki: 12, select: 0 }
  }
]

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
 * @param {Record<string, string>} [headers]
 */
async function rpc(request, method, params, headers = {}) {
  const response = await request.post(backend, {
    headers,
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
  assert.ok(stored, `${slug} template required`)
  assert.ok(Number.isSafeInteger(stored.revision_id), `${slug} revision required`)
  assert.equal(typeof stored.wikitext, "string", `${slug} source required`)
  return { revision: stored.revision_id, source: stored.wikitext }
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} slug
 * @param {string} token
 * @returns {Promise<import("../../src/lib/form-editor").PageForm>}
 */
async function readCreationForm(request, slug, token) {
  const permission = await rpc(
    request,
    "page_create_permission",
    {},
    {
      "X-Deepwell-Site-Id": String(siteId),
      "X-Deepwell-Page": slug,
      "X-Deepwell-Session-Token": token
    }
  )
  assert.equal(permission.can_create, true, `${slug} create permission required`)
  assert.ok(permission.form, `${slug} category form required`)
  return permission.form
}

/**
 * @param {import("../../src/lib/form-editor").FormField[]} fields
 * @param {Record<string, number>} counts
 */
function assertSchema(fields, counts) {
  assert.equal(
    fields.length,
    Object.values(counts).reduce((sum, count) => sum + count, 0)
  )
  assert.equal(new Set(fields.map((field) => field.name)).size, fields.length)
  for (const kind of ["static", "text", "wiki", "select"]) {
    assert.equal(
      fields.filter((field) => field.kind === kind).length,
      counts[kind],
      `${kind} count`
    )
  }
}

/**
 * @param {import("@playwright/test").Locator} wrapper
 * @param {import("../../src/lib/form-editor").FormField} field
 * @param {string} label
 */
async function assertStaticField(wrapper, field, label) {
  await expect(wrapper.locator(".static-label")).toHaveCount(label ? 1 : 0)
  if (label) {
    assert.equal(
      digest(await wrapper.locator(".static-label").textContent()),
      digest(label),
      `${field.name} static label digest`
    )
  }
  const content = wrapper.locator(".static-field")
  await expect(content).toHaveCount(1)
  assert.equal(
    digest(await content.textContent()),
    digest(text(field.properties.value ?? field.properties.default)),
    `${field.name} static text digest`
  )
  assert.equal(await content.locator("*").count(), 0, `${field.name} static text escaped`)
}

/**
 * @param {import("@playwright/test").Locator} wrapper
 * @param {import("../../src/lib/form-editor").FormField} field
 * @param {string} visibleLabel
 */
async function assertRadioField(wrapper, field, visibleLabel) {
  const legend = wrapper.locator("legend")
  await expect(legend).toHaveCount(visibleLabel ? 1 : 0)
  if (visibleLabel) {
    assert.equal(
      digest(await legend.textContent()),
      digest(visibleLabel),
      `${field.name} legend digest`
    )
  }
  await expect(wrapper.locator("fieldset")).toHaveAccessibleName(
    visibleLabel || field.name
  )
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
  assert.equal(
    digest(radios.map(({ code, label: optionLabel }) => ({ code, label: optionLabel }))),
    digest(
      field.options.map((option) => ({
        code: text(option.code),
        label: text(option.label)
      }))
    ),
    `${field.name} radio options in schema order`
  )
  const value = field.properties.default
  assert.equal(
    digest(radios.filter((radio) => radio.checked).map((radio) => radio.code)),
    digest(field.options.some((option) => option.code === value) ? [text(value)] : []),
    `${field.name} selected radio`
  )
}

/**
 * @param {import("@playwright/test").Locator} control
 * @param {import("../../src/lib/form-editor").FormField} field
 */
async function assertSelectField(control, field) {
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
  const value = field.properties.default
  const unknown = !field.options.some((option) => option.code === value)
  assert.equal(
    digest(options),
    digest(unknown ? [{ code: text(value), label: text(value) }, ...declared] : declared),
    `${field.name} options in schema order`
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
async function assertGuidance(wrapper, field, control) {
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
  if (field.kind === "text" || field.kind === "wiki") {
    const hint = text(field.properties.hint)
    if (hint) await expect(control).toHaveAttribute("placeholder", hint)
    else await expect(control).not.toHaveAttribute("placeholder")
  }
}

/**
 * @param {import("@playwright/test").Locator} control
 * @param {import("../../src/lib/form-editor").FormField} field
 */
async function assertTextField(control, field) {
  assert.equal(
    digest(await control.inputValue()),
    digest(text(field.properties.default)),
    `${field.name} default`
  )
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
 */
async function assertField(wrapper, field) {
  const label = text(field.properties.label)
  if (field.kind === "static") return assertStaticField(wrapper, field, label)
  const accessibleName = label || field.name
  if (field.kind === "select" && field.options.length >= 2 && field.options.length <= 4) {
    await assertRadioField(wrapper, field, label)
    await assertGuidance(wrapper, field, wrapper.locator("fieldset"))
    return
  }
  const visibleLabel = wrapper.locator("label")
  await expect(visibleLabel).toHaveCount(label ? 1 : 0)
  if (label) {
    assert.equal(
      digest(await visibleLabel.textContent()),
      digest(label),
      `${field.name} label digest`
    )
  }
  const control = wrapper.locator(controlSelector(field))
  await expect(control).toHaveCount(1)
  await expect(control).toHaveAccessibleName(accessibleName)
  if (field.kind === "select") await assertSelectField(control, field)
  else await assertTextField(control, field)
  await assertGuidance(wrapper, field, control)
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("../../src/lib/form-editor").FormField[]} fields
 */
async function assertForm(page, fields) {
  const editor = page.locator("#editor")
  await expect(editor).toBeVisible()
  await expect(editor.locator('textarea[name="wikitext"], .editor-wikitext')).toHaveCount(
    0
  )
  const rows = editor.locator(".form-field")
  await expect(rows).toHaveCount(fields.length)
  for (const [index, field] of fields.entries()) {
    await assertField(rows.nth(index), field)
  }
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {string} slug
 * @param {string} templateSlug
 * @param {{ revision: number; source: string }} baseline
 */
async function assertNoWrites(request, token, slug, templateSlug, baseline) {
  assert.equal(
    (await readView(request, slug, token))?.type,
    "missing",
    "target remains missing"
  )
  const after = await readStored(request, templateSlug)
  assert.equal(after.revision, baseline.revision, "template revision unchanged")
  assert.equal(digest(after.source), digest(baseline.source), "template source unchanged")
}

for (const category of categories) {
  test(`${category.name} missing-page form previews without writes`, async () => {
    const adminPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
    assert.ok(adminPath, "cobalt-import password file required")
    const suffix = randomBytes(8).toString("hex")
    const slug = `${category.name}:local-form-inventory-${suffix}`
    const title = `Local ${category.name} form inventory ${suffix}`
    const templateSlug = `${category.name}:_template`
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
        const template = await readStored(request, templateSlug)
        const form = await readCreationForm(request, slug, token)
        const fields = form.schema.fields
        assertSchema(fields, category.counts)
        if (category.name === "chain" || category.name === "arc") {
          const summaryName = category.name === "chain" ? "summary" : "arcSummary"
          const summary = fields.find((candidate) => candidate.name === summaryName)
          assert.ok(summary, `${category.name} ${summaryName} required`)
          assert.equal(summary.kind, "text", `${summaryName} text kind`)
          assert.equal(
            Number(summary.properties.height),
            3,
            `${summaryName} archived height`
          )
        }
        if (category.name === "npc") {
          const race = fields.find((candidate) => candidate.name === "race")
          assert.ok(race, "npc race field required")
          assert.equal(
            race.options.find((option) => option.code === "orc")?.label,
            "Orc:",
            "npc orc race label"
          )
        }
        assert.deepEqual(form.values, {}, "missing-page form values must be empty")
        const field = fields.find((candidate) => candidate.name === category.markerField)
        assert.ok(field, `${category.name} ${category.markerField} marker field required`)
        assert.ok(
          field.kind === "wiki" || field.kind === "text",
          `${category.name} ${category.markerField} marker field must be editable text`
        )
        await assertNoWrites(request, token, slug, templateSlug, template)

        try {
          const path = `/${slug}/edit/true/title/${encodeURIComponent(title)}`
          const navigation = await page.goto(`${preview}${path}`, {
            waitUntil: "networkidle"
          })
          assert.equal(
            navigation?.status(),
            404,
            `${category.name} missing-page HTTP status`
          )
          await expect(page.locator('#editor [name="title"]')).toHaveValue(title)
          await assertForm(page, fields)

          const marker = `Local ${category.name} preview ${randomBytes(8).toString("hex")}`
          const control = page.locator("#editor .form-field").nth(fields.indexOf(field))
          await control.locator(controlSelector(field)).fill(marker)
          const previewResponse = page.waitForResponse(
            (response) =>
              response.request().method() === "POST" &&
              response.url().includes(`/${slug}/edit`) &&
              response.url().endsWith("?/preview")
          )
          await page.locator("#edit-preview-button").click()
          const response = await previewResponse
          assert.equal(response.status(), 200, "preview HTTP status")
          assert.ok(
            (await response.text()).includes(marker),
            "preview response contains edited field"
          )
          const region = page.locator('section[aria-label="Page preview"]')
          await expect(region).toHaveAttribute("aria-busy", "false")
          await expect
            .poll(async () => (await region.textContent())?.includes(marker) ?? false)
            .toBe(true)
          await expect(control.locator(controlSelector(field))).toHaveValue(marker)
          await page.reload({ waitUntil: "networkidle" })
          await expect(page.locator('#editor [name="title"]')).toHaveValue(title)
          await assertForm(page, fields)
        } finally {
          await assertNoWrites(request, token, slug, templateSlug, template)
        }
      } finally {
        await context.close()
      }
    } finally {
      await browser.close()
    }
  })
}
