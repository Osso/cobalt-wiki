import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { open, readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000
const errorFile = process.env.COBALT_REMAINING_ACTION_ERROR_FILE
/** @param {string | null} value */
const digest = (value) => {
  assert.ok(typeof value === "string", "text content must exist")
  return createHash("sha256").update(value).digest("hex")
}

/** @typedef {import("../../src/lib/server/deepwell/views").PageView} PageView */
/** @typedef {Extract<PageView, { type: "found" }>["data"]} StoredPage */
/** @typedef {import("../../src/lib/server/load/page-backlinks").PageBacklinks} Backlinks */
/** @typedef {import("../../src/lib/server/deepwell/pageFile").PageFile} PageFile */
/**
 * @typedef {{
 *   sacrificial: true
 *   siteId: 6000000
 *   siteSlug: "cobalt-company"
 *   databaseLabel: "cobalt_local_full"
 *   username: "cobalt-import"
 *   existingSlug: string
 *   missingSlug?: string
 * }} Fixture
 */

/** @param {string} path */
async function readFixture(path) {
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(path, "utf8"))
  assert.equal(fixture.sacrificial, true)
  assert.equal(fixture.siteId, siteId)
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.username, "cobalt-import")
  assert.match(fixture.existingSlug, /^local-create-proof:roundtrip-[a-z0-9-]{8,}$/)
  if (fixture.missingSlug) {
    assert.match(fixture.missingSlug, /^local-preview-proof:[a-z0-9-]{8,}$/)
    assert.notEqual(fixture.missingSlug, fixture.existingSlug)
  }
  return fixture
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} method
 * @param {Record<string, unknown>} params
 * @param {string} token
 * @returns {Promise<unknown>}
 */
async function rpc(request, method, params, token) {
  const response = await request.post(backend, {
    headers: {
      "X-Deepwell-Site-Id": String(siteId),
      "X-Deepwell-Session-Token": token
    },
    data: { jsonrpc: "2.0", id: 1, method, params }
  })
  assert.equal(response.status(), 200, `${method} HTTP status`)
  /** @type {{ result?: unknown; error?: { code: number } }} */
  const payload = await response.json()
  assert.ok(!payload.error, `${method} RPC code ${payload.error?.code ?? "?"}`)
  return payload.result
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {string | null} slug
 * @returns {Promise<StoredPage>}
 */
async function readPage(request, token, slug) {
  const route = slug === null ? null : { slug, extra: "" }
  const result = /** @type {PageView} */ (
    await rpc(
      request,
      "page_view",
      { site_id: siteId, locales: ["en"], session_token: token, route },
      token
    )
  )
  assert.equal(result?.type, "found", `${slug ?? "root"} page must exist`)
  if (result.type !== "found") assert.fail("page_view must return a found page")
  return result.data
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {string} password
 */
async function login(context, password) {
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  await expect(page.locator("#login")).toBeVisible()
  await page.locator('#login [name="nameOrEmail"]').fill("cobalt-import")
  await page.locator('#login [name="password"]').fill(password)
  await page.locator('#login button[type="submit"]').click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (entry) => entry.name === "wikijump_token" && entry.domain === "127.0.0.1"
  )
  assert.ok(cookie, "authenticated session cookie required")
  return { page, token: decodeURIComponent(cookie.value) }
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {Fixture} fixture
 * @param {boolean} allowSave
 */
async function guardBrowserWrites(context, fixture, allowSave) {
  /** @type {string[]} */
  const blocked = []
  /** @type {string[]} */
  const saves = []
  await context.route("**/*", async (route) => {
    const request = route.request()
    if (["GET", "HEAD", "OPTIONS"].includes(request.method())) return route.continue()
    const url = new URL(request.url())
    const action = url.search
    const loginPost = url.pathname === "/-/login" && ["", "?/login"].includes(action)
    const readablePath = [
      `/${fixture.existingSlug}`,
      "/home:start",
      "/badges",
      "/home:_public",
      "/"
    ].includes(url.pathname)
    const readPost =
      readablePath &&
      [
        "?/backlinks",
        "?/fileList",
        "?/parentGet",
        "?/editorPages",
        "?/blockGet"
      ].includes(action)
    const fixtureSave =
      url.pathname === `/${fixture.existingSlug}` && action === "?/parentSet"
    if (request.method() === "POST" && url.origin === origin && (loginPost || readPost)) {
      return route.continue()
    }
    if (
      request.method() === "POST" &&
      url.origin === origin &&
      allowSave &&
      fixtureSave
    ) {
      saves.push(request.url())
      return route.continue()
    }
    blocked.push(`${request.method()} ${url.origin}${url.pathname}${action}`)
    return route.abort()
  })
  return { blocked, saves }
}

/** @param {import("@playwright/test").Page} page */
async function openOptions(page) {
  const options = page.locator("#page-options-bottom-2")
  await expect(options).toHaveCount(0)
  await expect(page.locator("#more-options-button")).toHaveText(/^\+ /)
  await page.locator("#more-options-button").click()
  await expect(options).toBeVisible()
  await expect(page.locator("#more-options-button")).toHaveText(/^- /)
  return options
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} source
 */
async function checkSourceAndPanels(page, source) {
  const options = await openOptions(page)
  await options.locator("#view-source-button").click()
  const sourcePane = page.locator("#action-area .page-source")
  await expect(sourcePane).toBeVisible()
  assert.equal(digest(await sourcePane.textContent()), digest(source), "source pane hash")
  await page.locator("#action-area .action-area-close").click()
  await expect(sourcePane).toHaveCount(0)
  await options.locator("#view-source-button").click()
  assert.equal(
    digest(await sourcePane.textContent()),
    digest(source),
    "reopened source hash"
  )
  await page.locator("#action-area .action-area-close").click()

  for (const [button, pane] of [
    ["rename-move-button", "page-move"],
    ["delete-button", "page-delete"],
    ["lock-page-button", "page-block-checkbox"],
    ["layout-button", "page-layout"]
  ]) {
    await options.locator(`#${button}`).click()
    await expect(page.locator(`#${pane}`)).toBeVisible()
    await page.locator("#action-area .action-area-close").click()
    await expect(page.locator(`#${pane}`)).toHaveCount(0)
  }
  await page.locator("#more-options-button").click()
  await expect(options).toHaveCount(0)
  await openOptions(page)
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {StoredPage} stored
 */
async function checkBacklinks(page, request, token, stored) {
  const expected = /** @type {Backlinks} */ (
    await rpc(
      request,
      "page_backlinks",
      { site_id: siteId, page_id: stored.page.page_id },
      token
    )
  )
  // The fixture has two raw links; viewer filtering can hide their identities.
  assert.ok(
    expected.links.length <= 2,
    "authorized links cannot exceed raw fixture links"
  )
  assert.deepEqual(expected.inclusions, [], "fixture has no direct inclusions")
  await page.locator("#backlinks-button").click()
  const pane = page.getByRole("region", { name: "Page backlinks" })
  await expect(pane.getByRole("heading", { name: "Backlinks" })).toBeVisible()
  for (const [index, items] of [expected.links, expected.inclusions].entries()) {
    const heading = index === 0 ? "Backlinks" : "Inclusions"
    const list = pane
      .locator("h2", { hasText: heading })
      .locator("xpath=following-sibling::ul[1]")
    if (!items.length) {
      await expect(
        pane.getByText(
          `No pages directly ${index === 0 ? "link to" : "include"} this page.`
        )
      ).toBeVisible()
      continue
    }
    await expect(list.locator("li")).toHaveCount(items.length)
    for (const [position, item] of items.entries()) {
      const link = list.locator("li a").nth(position)
      await expect(link).toHaveText(`${item.title || item.slug} (${item.slug})`)
      await expect(link).toHaveAttribute("href", `/${item.slug}`)
    }
  }
  await page.locator("#action-area .action-area-close").click()
  await expect(pane).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} slug
 */
async function visit(page, slug) {
  const response = await page.goto(`${origin}/${slug}`, { waitUntil: "networkidle" })
  assert.equal(response?.status(), 200, `${slug || "root"} HTTP status`)
}

/** @param {import("@playwright/test").Locator} locator */
async function normalizedBody(locator) {
  return locator.evaluate((node) => {
    const clone = node.cloneNode(true)
    const walker = document.createTreeWalker(clone, NodeFilter.SHOW_ALL)
    const blank = []
    while (walker.nextNode()) {
      const current = walker.currentNode
      if (
        current.nodeType === Node.COMMENT_NODE ||
        (current.nodeType === Node.TEXT_NODE && !current.textContent?.trim())
      ) {
        blank.push(current)
      }
    }
    blank.forEach((text) => text.parentNode?.removeChild(text))
    return /** @type {Element} */ (clone).innerHTML
  })
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} slug
 * @param {string} canonicalSlug
 */
async function checkPrint(page, slug, canonicalSlug) {
  await visit(page, slug)
  const title = await page.title()
  const normalHash = digest(await normalizedBody(page.locator("#page-content")))
  await openOptions(page)
  const printLink = page.locator("#print-page-button")
  await expect(printLink).toHaveAttribute("target", "_blank")
  await expect(printLink).toHaveAttribute("href", `/printer--friendly/${canonicalSlug}`)
  const [printed] = await Promise.all([page.waitForEvent("popup"), printLink.click()])
  try {
    await printed.waitForLoadState("networkidle")
    assert.equal(new URL(printed.url()).pathname, `/printer--friendly/${canonicalSlug}`)
    assert.equal(await printed.title(), title, "print title")
    assert.equal(
      digest(await normalizedBody(printed.locator(".print-body"))),
      normalHash,
      "print body DOM hash"
    )
    await expect(printed.locator(".source a")).toHaveAttribute(
      "href",
      `${origin}/${canonicalSlug}`
    )
    await expect(
      printed.locator(
        "#page-options-bottom, #action-area, #side-bar, #top-bar, #edit-button"
      )
    ).toHaveCount(0)
    await expect(printed.getByRole("button", { name: "Print the page" })).toBeVisible()
  } finally {
    await printed.close()
  }
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} missingSlug
 */
async function checkMissingPrint(request, missingSlug) {
  const response = await request.get(`${origin}/printer--friendly/${missingSlug}`)
  assert.equal(response.status(), 404, "known missing fixture print route")
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {StoredPage} badges
 */
async function checkFiles(page, request, token, badges) {
  const files = /** @type {PageFile[]} */ (
    await rpc(
      request,
      "page_get_files",
      { site_id: siteId, page_id: badges.page.page_id, deleted: false },
      token
    )
  )
  const active = files.filter((file) => file.revision_type !== "delete")
  assert.ok(active.length > 0, "badges file fixture must have stored files")
  await page.locator("#files-button").click()
  const pane = page.locator("#action-area .file-panel")
  await expect(pane).toBeVisible()
  const rows = pane.locator(".file-row")
  await expect(rows).toHaveCount(files.length)
  for (const file of files) {
    const row = pane.locator(`.file-row[data-id="${file.file_id}"]`)
    await expect(row.locator(".name a")).toHaveText(file.name)
    await expect(row.locator(".size")).toHaveAttribute(
      "title",
      `${file.size.toLocaleString("en-US")} Bytes`
    )
    await expect(row.locator(".size")).toHaveText(/^\d+(\.\d)? (B|KB|MB|GB)$/)
    if (file.revision_type === "delete") continue
    await row.locator(".file-information summary").click()
    const details = row.locator(".file-information-content dl")
    for (const [label, value] of [
      ["File name", file.name],
      ["File size", `${file.size.toLocaleString("en-US")} Bytes`],
      ["MIME type", file.mime],
      ["Local file created", new Date(file.file_created_at).toLocaleString()]
    ]) {
      await expect(
        details
          .locator("dt", { hasText: label })
          .locator("xpath=following-sibling::dd[1]")
      ).toHaveText(value)
    }
    const fileUrl = await row.locator(".name a").getAttribute("href")
    assert.ok(fileUrl, "listed file URL required")
    await expect(
      details
        .locator("dt", { hasText: "Full file URL" })
        .locator("xpath=following-sibling::dd[1]/a")
    ).toHaveAttribute("href", fileUrl)
    if (file.revision_comments && !file.hidden_fields.includes("comments")) {
      await expect(
        details
          .locator("dt", { hasText: "Revision comment" })
          .locator("xpath=following-sibling::dd[1]")
      ).toHaveText(file.revision_comments)
    }
    await row.locator(".file-information summary").click()
  }
  const total = active.reduce((sum, file) => sum + file.size, 0)
  await expect(
    pane.getByText(`Total files size: ${total.toLocaleString("en-US")} Bytes`)
  ).toBeVisible()
  await page.locator("#action-area .action-area-close").click()
  await expect(pane).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {StoredPage} stored
 */
async function readParents(request, token, stored) {
  return /** @type {string[]} */ (
    await rpc(
      request,
      "parent_get_all",
      { site_id: siteId, page: stored.page.page_id },
      token
    )
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").BrowserContext} context
 * @param {string[]} parents
 */
async function checkParentLoading(page, context, parents) {
  const pending = Promise.withResolvers()
  const routePattern = /\?\/parentGet$/
  /** @param {import("@playwright/test").Route} route */
  const holdResponse = async (route) => {
    await pending.promise
    await route.fallback()
  }
  await context.route(routePattern, holdResponse)
  try {
    await page.locator("#parent-page-button").click()
    const form = page.locator("#page-parent")
    const input = form.getByLabel("Parent page names")
    await expect(input).toBeDisabled()
    await expect(form.locator('input[type="submit"]')).toBeDisabled()
    await expect(form.locator('input[value="Clear parents"]')).toBeDisabled()
    pending.resolve(undefined)
    await expect(input).toBeEnabled()
    await expect(input).toHaveValue(parents.join(" "))
    await expect(form.locator('input[type="submit"]')).toBeEnabled()
    await page.locator("#action-area .action-area-close").click()
  } finally {
    pending.resolve(undefined)
    await context.unroute(routePattern, holdResponse)
  }
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string[]} parents
 */
async function openParent(page, parents) {
  await page.locator("#parent-page-button").click()
  const form = page.locator("#page-parent")
  await expect(form.getByLabel("Parent page names")).toBeVisible()
  await expect(form).toContainText("Enter multiple parent page names")
  const input = form.getByLabel("Parent page names")
  await expect(input).toBeEnabled()
  await expect(input).toHaveValue(parents.join(" "))
  return { form, input }
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {StoredPage} stored
 * @param {string[]} baseline
 */
async function checkParentDraft(page, request, token, stored, baseline) {
  const { form, input } = await openParent(page, baseline)
  await input.fill("draft-that-must-not-save")
  await form.locator('input[value="Clear parents"]').click()
  await expect(input).toHaveValue("")
  await input.fill("draft-that-must-not-save")
  await form.locator('input.btn-danger[type="button"]').click()
  await expect(form).toHaveCount(0)
  assert.deepEqual(await readParents(request, token, stored), baseline, "draft unchanged")
  await openParent(page, baseline)
  await page.locator("#action-area .action-area-close").click()
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string[]} expected
 */
async function submitParents(page, expected) {
  const form = page.locator("#page-parent")
  await form.getByLabel("Parent page names").fill(expected.join(" "))
  const response = page.waitForResponse((item) => item.url().endsWith("?/parentSet"))
  await form.locator('input[type="submit"]').click()
  assert.equal((await response).status(), 200, "parentSet HTTP status")
  await expect(form).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {Fixture} fixture
 * @param {StoredPage} stored
 * @param {string[]} baseline
 */
async function saveAndRestoreParents(page, request, token, fixture, stored, baseline) {
  const expected = [...new Set([...baseline, "home:start", "badges"])]
  assert.ok(
    expected.length > baseline.length,
    "fixture must need at least one new parent"
  )
  try {
    await openParent(page, baseline)
    await submitParents(page, expected)
    await expect
      .poll(async () => [...(await readParents(request, token, stored))].sort())
      .toEqual([...expected].sort())
    const savedParents = await readParents(request, token, stored)
    await visit(page, fixture.existingSlug)
    await openOptions(page)
    const reopened = await openParent(page, savedParents)
    await expect(reopened.input).toHaveValue(savedParents.join(" "))
    await reopened.form.locator('input.btn-danger[type="button"]').click()
  } finally {
    const current = await readParents(request, token, stored)
    if ([...current].sort().join(" ") !== [...baseline].sort().join(" ")) {
      await visit(page, fixture.existingSlug)
      await openOptions(page)
      await openParent(page, current)
      await submitParents(page, baseline)
    }
    assert.deepEqual(
      [...(await readParents(request, token, stored))].sort(),
      [...baseline].sort(),
      "parents restored"
    )
  }
}

/** @param {string} stage @param {unknown} error */
async function reportFailure(stage, error) {
  assert.ok(errorFile, "private failure file required")
  const detail = `${stage}: ${error instanceof Error ? error.stack : String(error)}\n`
  const handle = await open(errorFile, "w", 0o600)
  try {
    await handle.chmod(0o600)
    await handle.writeFile(detail)
  } finally {
    await handle.close()
  }
}

test(
  "remaining local page actions use authorized data without unapproved writes",
  { timeout: 90_000 },
  async () => {
    const fixturePath = process.env.COBALT_PAGE_PREVIEW_FIXTURE
    const passwordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
    assert.ok(
      fixturePath && passwordPath && errorFile,
      "protected fixture, admin password, private error path required"
    )
    assert.ok(!process.env.COBALT_PARENT_SAVE || process.env.COBALT_PARENT_SAVE === "1")
    const allowSave = process.env.COBALT_PARENT_SAVE === "1"
    let browser
    let stage = "fixture"
    try {
      const fixture = await readFixture(fixturePath)
      const gatewayPath = process.env.COBALT_LOCAL_PASSWORD_FILE
      const gatewayPassword = gatewayPath
        ? (await readFile(gatewayPath, "utf8")).trim()
        : null
      const password = (await readFile(passwordPath, "utf8")).trim()
      browser = await chromium.launch({
        executablePath: "/usr/bin/chromium",
        headless: true
      })
      const context = await browser.newContext({
        ...(gatewayPassword && {
          httpCredentials: { username: "cobalt", password: gatewayPassword, origin }
        })
      })
      try {
        const { blocked, saves } = await guardBrowserWrites(context, fixture, allowSave)
        stage = "login"
        const { page, token } = await login(context, password)
        const request = context.request
        stage = "source and readonly panels"
        const protectedPage = await readPage(request, token, fixture.existingSlug)
        await visit(page, fixture.existingSlug)
        await checkSourceAndPanels(page, protectedPage.wikitext)
        stage = "home:start backlinks"
        const home = await readPage(request, token, "home:start")
        await visit(page, "home:start")
        await openOptions(page)
        await checkBacklinks(page, request, token, home)
        stage = "print named and root routes"
        await checkPrint(page, "home:start", "home:start")
        const root = await readPage(request, token, null)
        await checkPrint(page, "", root.page.slug)
        if (fixture.missingSlug) await checkMissingPrint(request, fixture.missingSlug)
        stage = "badges file listing"
        const badges = await readPage(request, token, "badges")
        await visit(page, "badges")
        await checkFiles(page, request, token, badges)
        stage = "parent draft and lookup"
        await visit(page, fixture.existingSlug)
        await openOptions(page)
        const baseline = await readParents(request, token, protectedPage)
        await checkParentLoading(page, context, baseline)
        await checkParentDraft(page, request, token, protectedPage, baseline)
        const { form, input } = await openParent(page, baseline)
        await input.fill("home:start ba")
        const matches = /** @type {{ slug: string; title: string }[]} */ (
          await rpc(request, "editor_pages", { query: "ba" }, token)
        )
        const badgesMatch = matches.find((match) => match.slug === "badges")
        assert.ok(badgesMatch, "authorized badges suggestion required")
        await expect(
          form.locator('datalist option[value="home:start badges"]')
        ).toHaveText(badgesMatch.title)
        await form.locator('input.btn-danger[type="button"]').click()
        if (allowSave) {
          stage = "parent save and restoration"
          await saveAndRestoreParents(
            page,
            request,
            token,
            fixture,
            protectedPage,
            baseline
          )
          assert.equal(saves.length, 2, "only fixture save and restore may write")
        } else {
          assert.deepEqual(saves, [], "read-only run cannot save parents")
        }
        assert.deepEqual(blocked, [], "browser must not attempt unauthorized writes")
      } finally {
        await context.close()
      }
    } catch (error) {
      await reportFailure(stage, error)
      assert.fail(`Remaining actions failed at ${stage}; details in private error file`)
    } finally {
      await browser?.close()
    }
  }
)
