import assert from "node:assert/strict"
import { randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { setTimeout as delay } from "node:timers/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const preview = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000
const formDefinition =
  "====\n[[form]]\nfields:\n  name:\n    type: text\n    label: Name\n[[/form]]"
const beforeTemplate = `Before marker: %%form_data{name}%%\n${formDefinition}`
const afterTemplate = `After marker: %%form_data{name}%%\n${formDefinition}`

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} slug
 */
async function readStoredPage(request, slug) {
  const response = await request.post(backend, {
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "page_get",
      params: {
        site_id: siteId,
        page: slug,
        details: { wikitext: true, compiled: true }
      }
    }
  })
  assert.equal(response.status(), 200, `page_get HTTP status for ${slug}`)
  const payload = await response.json()
  assert.ok(!payload.error, `page_get ${slug}: ${payload.error?.message ?? "RPC error"}`)
  return payload.result
}

/**
 * @param {import("@playwright/test").Page} page @param {string} slug
 * @param {string} title @param {string} source
 */
async function createRawPage(page, slug, title, source) {
  await page.goto(`${preview}/${slug}`, { waitUntil: "networkidle" })
  await page.locator(`a[href="/${slug}/edit"]`).click()
  const editor = page.locator("#editor")
  await expect(editor).toBeVisible()
  await editor.locator('[name="title"]').fill(title)
  await editor.locator('[name="wikitext"]').fill(source)
  await saveEditor(page, slug)
}

/** @param {import("@playwright/test").Page} page @param {string} slug */
async function saveEditor(page, slug) {
  const responsePromise = page.waitForResponse(
    (response) =>
      response.request().method() === "POST" &&
      response.url().includes(`/${slug}/edit`) &&
      response.url().includes("?/edit")
  )
  await page.locator('#editor [type="submit"]').click()
  assert.equal((await responsePromise).status(), 200, `${slug} save status`)
  await expect(page.locator("#editor")).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} slug
 */
async function requireStoredPage(request, slug) {
  const stored = await readStoredPage(request, slug)
  assert.ok(stored, `${slug} must have a stored page`)
  assert.ok(Number.isSafeInteger(stored.revision_id), `${slug} revision ID required`)
  assert.equal(typeof stored.wikitext, "string", `${slug} source required`)
  assert.equal(
    typeof stored.compiled_body_html,
    "string",
    `${slug} compiled HTML required`
  )
  return stored
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} slug @param {string} originalSource @param {number}
 *   revisionId
 */
async function waitForWorkerRefresh(request, slug, originalSource, revisionId) {
  const started = performance.now()
  const deadline = started + 5_000
  while (performance.now() < deadline) {
    const stored = await requireStoredPage(request, slug)
    assert.equal(stored.wikitext, originalSource, "worker must preserve dependent source")
    assert.equal(
      stored.revision_id,
      revisionId,
      "worker must preserve dependent revision"
    )
    if (
      stored.compiled_body_html.includes("After marker:") &&
      stored.compiled_body_html.includes("Moth") &&
      !stored.compiled_body_html.includes("Before marker:")
    ) {
      console.log(
        JSON.stringify({ slug, refreshMs: Math.round(performance.now() - started) })
      )
      return stored
    }
    await delay(250)
  }
  assert.fail(`${slug} stored compiled HTML did not refresh within 5 seconds`)
}

test("template edit refreshes only dependent stored compiled HTML through worker", async () => {
  assert.equal(
    process.env.COBALT_TEMPLATE_REFRESH_FIXTURE,
    "true",
    "explicit COBALT_TEMPLATE_REFRESH_FIXTURE=true required"
  )
  const passwordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(passwordPath, "protected COBALT_LOCAL_ADMIN_PASSWORD_FILE required")
  const adminPassword = (await readFile(passwordPath, "utf8")).trim()
  assert.ok(adminPassword, "local admin password required")

  const suffix = randomBytes(8).toString("hex")
  const category = `local-template-proof-${suffix}`
  const templateSlug = `${category}:_template`
  const dependentSlug = `${category}:sample`
  const unrelatedSlug = `local-template-control-${suffix}:sample`
  // Log only synthetic identities; leave created pages in place for inspection and safe retries.
  console.log(JSON.stringify({ templateSlug, dependentSlug, unrelatedSlug }))

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
      await page.locator('#login [name="password"]').fill(adminPassword)
      await page.locator("#login button[type=submit]").click()
      await expect(page.locator("#login")).toHaveCount(0)
      const cookie = (await context.cookies()).find(
        (entry) => entry.name === "wikijump_token" && entry.domain === "127.0.0.1"
      )
      assert.ok(cookie, "authenticated browser session required")

      for (const slug of [templateSlug, dependentSlug, unrelatedSlug]) {
        assert.equal(
          await readStoredPage(context.request, slug),
          null,
          `${slug} must be absent`
        )
      }

      await createRawPage(page, templateSlug, `Template ${suffix}`, beforeTemplate)
      const firstTemplate = await requireStoredPage(context.request, templateSlug)
      assert.equal(firstTemplate.wikitext, beforeTemplate)
      console.log(
        JSON.stringify({ slug: templateSlug, revisionId: firstTemplate.revision_id })
      )

      await page.goto(`${preview}/${dependentSlug}/edit`, { waitUntil: "networkidle" })
      const editor = page.locator("#editor")
      await expect(editor).toBeVisible()
      await expect(editor.locator('[name="wikitext"]')).toHaveCount(0)
      await editor.locator('[name="title"]').fill(`Sample ${suffix}`)
      await editor.getByLabel("Name", { exact: true }).fill("Moth")
      await saveEditor(page, dependentSlug)
      const dependent = await requireStoredPage(context.request, dependentSlug)
      assert.ok(dependent.wikitext.includes("Moth"), "form source must store Moth")
      assert.ok(
        dependent.compiled_body_html.includes("Before marker:") &&
          dependent.compiled_body_html.includes("Moth"),
        "stored dependent HTML must first show Before marker and Moth"
      )
      console.log(
        JSON.stringify({ slug: dependentSlug, revisionId: dependent.revision_id })
      )

      const controlSource = `+ Unrelated control ${suffix}`
      await createRawPage(page, unrelatedSlug, `Control ${suffix}`, controlSource)
      const unrelated = await requireStoredPage(context.request, unrelatedSlug)
      assert.equal(unrelated.wikitext, controlSource)
      console.log(
        JSON.stringify({ slug: unrelatedSlug, revisionId: unrelated.revision_id })
      )

      await page.goto(`${preview}/${templateSlug}/edit`, { waitUntil: "networkidle" })
      await expect(page.locator('#editor [name="wikitext"]')).toHaveValue(beforeTemplate)
      await page.locator('#editor [name="wikitext"]').fill(afterTemplate)
      await saveEditor(page, templateSlug)
      const editedTemplate = await requireStoredPage(context.request, templateSlug)
      assert.equal(editedTemplate.wikitext, afterTemplate)
      assert.notEqual(editedTemplate.revision_id, firstTemplate.revision_id)
      console.log(
        JSON.stringify({ slug: templateSlug, revisionId: editedTemplate.revision_id })
      )

      await waitForWorkerRefresh(
        context.request,
        dependentSlug,
        dependent.wikitext,
        dependent.revision_id
      )
      const unchanged = await requireStoredPage(context.request, unrelatedSlug)
      assert.equal(unchanged.wikitext, unrelated.wikitext, "control source unchanged")
      assert.equal(
        unchanged.revision_id,
        unrelated.revision_id,
        "control revision unchanged"
      )
      assert.equal(
        unchanged.compiled_body_html,
        unrelated.compiled_body_html,
        "control stored compiled HTML unchanged"
      )

      await page.goto(`${preview}/${dependentSlug}`, { waitUntil: "networkidle" })
      await page.reload({ waitUntil: "networkidle" })
      await expect(page.locator("#page-content")).toContainText("After marker: Moth")
      await expect(page.locator("#page-content")).not.toContainText("Before marker:")
    } finally {
      await context.close()
    }
  } finally {
    await browser.close()
  }
})
