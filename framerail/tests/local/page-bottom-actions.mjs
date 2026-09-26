import assert from "node:assert/strict"
import { createHash, randomBytes } from "node:crypto"
import { readFile, writeFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

// Default: no browser writes after login. Opt in to fixture-only save/restore with
// COBALT_BOTTOM_TAG_SAVE=1; the runner must explicitly authorize that mutation.
const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const errorFile = process.env.COBALT_BOTTOM_ACTION_ERROR_FILE

/** @typedef {import("../../src/lib/server/deepwell/views").PageView} PageView */
/** @typedef {Extract<PageView, { type: "found" }>["data"]} StoredPage */
/**
 * @typedef {{
 *   sacrificial: true
 *   siteId: 6000000
 *   siteSlug: "cobalt-company"
 *   databaseLabel: "cobalt_local_full"
 *   username: "cobalt-import"
 *   existingSlug: string
 * }} Fixture
 */

/** @param {string} path */
async function readFixture(path) {
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(path, "utf8"))
  assert.equal(fixture.sacrificial, true, "protected sacrificial fixture required")
  assert.equal(fixture.siteId, 6000000)
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.username, "cobalt-import")
  assert.match(fixture.existingSlug, /^local-create-proof:roundtrip-[a-z0-9-]{8,}$/)
  return fixture
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {Fixture} fixture
 * @param {string} token
 */
async function readStoredPage(request, fixture, token) {
  const response = await request.post(backend, {
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "page_view",
      params: {
        site_id: fixture.siteId,
        locales: ["en"],
        session_token: token,
        route: { slug: fixture.existingSlug, extra: "" }
      }
    }
  })
  assert.equal(response.status(), 200, "fixture page_view HTTP status")
  const payload = await response.json()
  assert.ok(!payload.error, `fixture page_view RPC error ${payload.error?.code ?? "?"}`)
  /** @type {PageView} */
  const view = payload.result
  assert.equal(view?.type, "found", "fixture page must exist")
  if (view.type !== "found") assert.fail("fixture page must exist")
  return view.data
}

/** @param {StoredPage} page */
function pageFingerprint(page) {
  return {
    source: createHash("sha256").update(page.wikitext).digest("hex"),
    revision: page.page_revision.revision_id,
    tags: [...page.page_revision.tags]
  }
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
  await page.locator("#login button[type=submit]").click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (entry) => entry.name === "wikijump_token" && entry.domain === "127.0.0.1"
  )
  assert.ok(cookie, "browser login must create a session")
  return { page, token: decodeURIComponent(cookie.value) }
}

/** @param {import("@playwright/test").Request} request */
async function submittedChanges(request) {
  const body = request.postDataBuffer()
  assert.ok(body, "setTags must send a multipart body")
  const formRequest = new Request(request.url(), {
    method: "POST",
    headers: request.headers(),
    body: new Uint8Array(body).buffer
  })
  const form = await formRequest.formData()
  const changes = form.get("changes")
  assert.ok(typeof changes === "string", "setTags changes field required")
  return changes
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {Fixture} fixture
 * @param {boolean} allowSave
 */
async function interceptWrites(context, fixture, allowSave) {
  /** @type {string[]} */
  const unexpectedPosts = []
  /** @type {string[]} */
  const capturedChanges = []
  await context.route("**/*", async (route) => {
    const request = route.request()
    if (["GET", "HEAD", "OPTIONS"].includes(request.method())) {
      return route.continue()
    }
    const url = new URL(request.url())
    const isFixtureSave =
      request.method() === "POST" &&
      url.origin === origin &&
      url.pathname === `/${fixture.existingSlug}` &&
      url.search === "?/setTags"
    if (!isFixtureSave) {
      unexpectedPosts.push(`${url.origin}${url.pathname}${url.search}`)
      return route.abort()
    }
    capturedChanges.push(await submittedChanges(request))
    if (allowSave) return route.continue()
    return route.fulfill({
      status: 403,
      contentType: "application/json",
      body: JSON.stringify({
        type: "failure",
        status: 403,
        data: { message: "Tag save intercepted by acceptance test" }
      })
    })
  })
  return { unexpectedPosts, capturedChanges }
}

/** @param {import("@playwright/test").Page} page */
async function assertBottomActions(page) {
  const actions = page.locator("#page-options-bottom")
  await expect(actions).toBeVisible()
  for (const [id, label] of [
    ["edit-button", /^Edit$/i],
    ["tags-button", /^Tags$/i],
    ["history-button", /^History$/i],
    ["files-button", /^Files$/i],
    ["more-options-button", /options/i]
  ]) {
    await expect(actions.locator(`#${id}`)).toBeVisible()
    await expect(actions.locator(`#${id}`)).toHaveText(label)
  }
  await expect(actions.locator("#pagerate-button")).toHaveCount(0)
  await expect(actions.getByText("Vote", { exact: true })).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string[]} tags
 */
async function openTags(page, tags) {
  await page.locator("#tags-button").click()
  const form = page.locator("#page-tags")
  await expect(form).toBeVisible()
  await expect(page.locator(".page-tags-header")).toHaveText("Page Tags")
  await expect(page.locator("#action-area p")).toContainText(
    "Tags are a nice way to organize content in your Site."
  )
  await expect(
    page.locator('#action-area a[href="http://en.wikipedia.org/wiki/Tags"]')
  ).toBeVisible()
  await expect(
    page.locator('#action-area a[href="http://en.wikipedia.org/wiki/Tag_cloud"]')
  ).toBeVisible()
  await expect(form.locator("table.form")).toBeVisible()
  const input = form.getByLabel("Tags:", { exact: true })
  await expect(input).toHaveAttribute("size", "50")
  await expect(input).toHaveValue([...tags].sort().join(" "))
  await expect(form.locator(".sub")).toHaveText("Space-separated list of tags.")
  for (const label of ["close", "clear", "save tags"]) {
    await expect(form.locator(`input[value="${label}"]`)).toBeVisible()
  }
  return { form, input }
}

/**
 * @param {string[]} before
 * @param {string} changes
 * @param {string[]} expected
 */
function assertTagChanges(before, changes, expected) {
  const actual = [...before]
  for (const word of changes.split(/\s+/).filter(Boolean)) {
    assert.match(word, /^[+-][^\s]+$/, "only signed tag changes allowed")
    const tag = word.slice(1)
    if (word.startsWith("-")) {
      assert.ok(actual.includes(tag), `removed tag ${tag} must exist`)
      actual.splice(actual.indexOf(tag), 1)
    } else {
      assert.ok(!actual.includes(tag), `added tag ${tag} must be new`)
      actual.push(tag)
    }
  }
  assert.deepEqual(
    [...actual].sort(),
    [...expected].sort(),
    "changes must produce exact set"
  )
  assert.deepEqual(
    changes.split(/\s+/).filter(Boolean).sort(),
    [
      ...before.filter((tag) => !expected.includes(tag)).map((tag) => `-${tag}`),
      ...expected.filter((tag) => !before.includes(tag)).map((tag) => `+${tag}`)
    ].sort(),
    "save must submit only the exact required additions/removals"
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string[]} baseline
 * @param {string[]} capturedChanges
 */
async function assertDraftControls(page, baseline, capturedChanges) {
  const { form, input } = await openTags(page, baseline)
  await input.fill("discard-this-draft")
  await form.locator('input[value="clear"]').click()
  await expect(input).toHaveValue("")
  assert.deepEqual(capturedChanges, [], "clear must not send setTags")
  await input.fill("discard-this-draft")
  await form.locator('input[value="close"]').click()
  await expect(form).toHaveCount(0)
  assert.deepEqual(capturedChanges, [], "close must not send setTags")
  return openTags(page, baseline)
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string[]} baseline
 * @param {string[]} capturedChanges
 */
async function assertInterceptedSave(page, baseline, capturedChanges) {
  const { form, input } = await assertDraftControls(page, baseline, capturedChanges)
  const marker = `local-bottom-actions-${randomBytes(6).toString("hex")}`
  assert.ok(!baseline.includes(marker), "new tag marker must not exist")
  const expected = [...baseline.slice(1), marker]
  await input.fill(expected.join(" "))
  await form.locator('input[value="save tags"]').click()
  await expect.poll(() => capturedChanges.length).toBe(1)
  assertTagChanges(baseline, capturedChanges[0], expected)
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").BrowserContext} context
 * @param {Fixture} fixture
 * @param {string} token
 * @param {ReturnType<typeof pageFingerprint>} baseline
 * @param {string[]} capturedChanges
 */
async function saveAndRestore(page, context, fixture, token, baseline, capturedChanges) {
  const marker = `local-bottom-actions-${randomBytes(6).toString("hex")}`
  assert.ok(!baseline.tags.includes(marker), "new tag marker must not exist")
  const expected = [...baseline.tags.slice(1), marker]
  const { form, input } = await openTags(page, baseline.tags)
  await input.fill(expected.join(" "))
  try {
    const savedResponse = page.waitForResponse((response) =>
      response.url().endsWith(`/${fixture.existingSlug}?/setTags`)
    )
    await form.locator('input[value="save tags"]').click()
    assert.equal((await savedResponse).status(), 200, "fixture tag save HTTP status")
    await expect
      .poll(async () => {
        const current = await readStoredPage(context.request, fixture, token)
        return [...current.page_revision.tags].sort()
      })
      .toEqual([...expected].sort())
    assertTagChanges(baseline.tags, capturedChanges[0], expected)
  } finally {
    const stored = await readStoredPage(context.request, fixture, token)
    const actual = stored.page_revision.tags
    if ([...actual].sort().join(" ") !== [...baseline.tags].sort().join(" ")) {
      await page.goto(`${origin}/${fixture.existingSlug}`, { waitUntil: "networkidle" })
      const reopened = await openTags(page, actual)
      await reopened.input.fill(baseline.tags.join(" "))
      const restoredResponse = page.waitForResponse((response) =>
        response.url().endsWith(`/${fixture.existingSlug}?/setTags`)
      )
      await reopened.form.locator('input[value="save tags"]').click()
      assert.equal(
        (await restoredResponse).status(),
        200,
        "fixture tag restore HTTP status"
      )
      await expect
        .poll(async () => {
          const restored = await readStoredPage(context.request, fixture, token)
          return [...restored.page_revision.tags].sort()
        })
        .toEqual([...baseline.tags].sort())
      assertTagChanges(actual, capturedChanges.at(-1), baseline.tags)
    }
  }
  const restored = pageFingerprint(await readStoredPage(context.request, fixture, token))
  assert.equal(restored.source, baseline.source, "save/restore must preserve source")
  assert.deepEqual(
    [...restored.tags].sort(),
    [...baseline.tags].sort(),
    "save/restore must preserve tags"
  )
}

/** @param {unknown} error */
async function reportFailure(error, stage) {
  const detail = `${stage}: ${error instanceof Error ? error.stack : String(error)}\n`
  if (errorFile) await writeFile(errorFile, detail, { mode: 0o600 })
  else process.stderr.write(detail)
}

test("local Wikidot bottom actions and Tags preserve fixture by default", async () => {
  const fixturePath = process.env.COBALT_PAGE_PREVIEW_FIXTURE
  const adminPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  const gatewayPath = process.env.COBALT_LOCAL_PASSWORD_FILE
  assert.ok(fixturePath && adminPath, "fixture and admin password file paths required")
  assert.ok(
    !process.env.COBALT_BOTTOM_TAG_SAVE || process.env.COBALT_BOTTOM_TAG_SAVE === "1"
  )
  const allowSave = process.env.COBALT_BOTTOM_TAG_SAVE === "1"
  const fixture = await readFixture(fixturePath)
  const adminPassword = (await readFile(adminPath, "utf8")).trim()
  const gatewayPassword = gatewayPath
    ? (await readFile(gatewayPath, "utf8")).trim()
    : null
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  let stage = "login"
  try {
    const context = await browser.newContext({
      ...(gatewayPassword && {
        httpCredentials: { username: "cobalt", password: gatewayPassword, origin }
      })
    })
    try {
      const { page, token } = await login(context, fixture, adminPassword)
      stage = "baseline page_view"
      const baseline = pageFingerprint(
        await readStoredPage(context.request, fixture, token)
      )
      stage = "browser action guard"
      const { unexpectedPosts, capturedChanges } = await interceptWrites(
        context,
        fixture,
        allowSave
      )
      stage = "Wikidot bottom actions"
      await page.goto(`${origin}/${fixture.existingSlug}`, { waitUntil: "networkidle" })
      await assertBottomActions(page)
      stage = "Tags clear, close, reopen, and save"
      if (allowSave) {
        const { form } = await assertDraftControls(page, baseline.tags, capturedChanges)
        await form.locator('input[value="close"]').click()
        await saveAndRestore(page, context, fixture, token, baseline, capturedChanges)
      } else {
        await assertInterceptedSave(page, baseline.tags, capturedChanges)
      }
      stage = "final stored page_view"
      const after = pageFingerprint(await readStoredPage(context.request, fixture, token))
      assert.equal(after.source, baseline.source, "source must remain unchanged")
      assert.deepEqual(
        [...after.tags].sort(),
        [...baseline.tags].sort(),
        "stored tags must remain unchanged"
      )
      if (!allowSave)
        assert.equal(after.revision, baseline.revision, "revision unchanged")
      assert.deepEqual(unexpectedPosts, [], "only fixture tag saves may POST after login")
    } finally {
      await context.close()
    }
  } catch (error) {
    await reportFailure(error, stage)
    throw error
  } finally {
    await browser.close()
  }
})
