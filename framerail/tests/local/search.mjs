import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect, request } from "@playwright/test"

const preview = "http://127.0.0.1:3090"
const searchPath = "/search:site"
const pageSize = 20

/**
 * @typedef {{
 *   sacrificial: true
 *   query: string
 *   expectedSlug: string
 *   expectedTitleSha256?: string
 *   expectedEscapedText?: string
 *   paginationQuery: string
 * }} Fixture
 */

/** @param {string} path */
async function readFixture(path) {
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(path, "utf8"))
  assert.equal(fixture.sacrificial, true, "prepared sacrificial search fixture required")
  assert.ok(fixture.query?.trim(), "fixture query required")
  assert.match(fixture.expectedSlug, /^[a-z0-9][a-z0-9:_-]*$/)
  assert.ok(fixture.paginationQuery?.trim(), "pagination query required")
  if (fixture.expectedTitleSha256) {
    assert.match(fixture.expectedTitleSha256, /^[a-f0-9]{64}$/)
  }
  return fixture
}

/**
 * @param {import("@playwright/test").APIResponse
 *   | import("@playwright/test").Response} response
 */
function assertNoindex(response) {
  assert.match(
    response.headers()["x-robots-tag"] ?? "",
    /(?:^|,)\s*noindex(?:\s*,|$)/i,
    "search gateway must prohibit indexing"
  )
}

/** @param {import("@playwright/test").Page} page */
async function readResultSlugs(page) {
  const links = page.locator("#main-content ul > li h2 a")
  return links.evaluateAll((elements) =>
    elements.map((element) => {
      if (!(element instanceof HTMLAnchorElement)) {
        throw new Error("search result must be a link")
      }
      return new URL(element.href).pathname
    })
  )
}

/** @param {import("@playwright/test").Page} page */
async function assertPlainTextResults(page) {
  const results = page.locator("#main-content ul > li")
  await expect(results.first()).toBeVisible()
  await expect(results.locator("script, iframe, img, svg")).toHaveCount(0)
  await expect(results.locator("p").first()).toBeVisible()
  const snippetsArePlainText = await results
    .locator("p")
    .evaluateAll((paragraphs) =>
      paragraphs.every((paragraph) => paragraph.childElementCount === 0)
    )
  assert.equal(snippetsArePlainText, true, "result snippets must render as text")
}

/** @param {import("@playwright/test").Page} page @param {Fixture} fixture */
async function assertExpectedResult(page, fixture) {
  const result = page.locator("#main-content ul > li").filter({
    has: page.locator(`h2 a[href="/${fixture.expectedSlug}"]`)
  })
  await expect(result).toHaveCount(1)
  if (fixture.expectedTitleSha256) {
    const title = await result.locator("h2 a").innerText()
    const hash = createHash("sha256").update(title).digest("hex")
    assert.equal(hash, fixture.expectedTitleSha256, "result title hash")
  }
  if (fixture.expectedEscapedText) {
    await expect(result.locator("p").first()).toContainText(fixture.expectedEscapedText)
  }
}

/** @param {import("@playwright/test").Page} page @param {string} query */
async function submitHeaderSearch(page, query) {
  const headerForm = page.locator("#search-top-box-form")
  await expect(headerForm).toBeVisible()
  await headerForm.locator('#search-top-box-input[name="query"]').fill(query)
  await headerForm.getByRole("button", { name: "Search" }).click()
  await expect(page).toHaveURL(
    (url) => url.pathname === searchPath && url.searchParams.get("query") === query
  )
  await expect(page.getByRole("heading", { name: "Search results" })).toBeVisible()
}

/** @param {import("@playwright/test").Page} page @param {Fixture} fixture */
async function assertPagination(page, fixture) {
  await submitHeaderSearch(page, fixture.paginationQuery)
  await assertPlainTextResults(page)
  const firstPage = await readResultSlugs(page)
  assert.equal(firstPage.length, pageSize, "fixture must match more than one full page")
  assert.equal(new Set(firstPage).size, pageSize, "first-page slugs must be unique")
  const pager = page.getByRole("navigation", { name: "Search results pages" })
  await expect(pager.getByRole("link", { name: "Previous" })).toHaveCount(0)
  await pager.getByRole("link", { name: "Next" }).click()
  await expect(page).toHaveURL(
    (url) =>
      url.pathname === searchPath &&
      url.searchParams.get("query") === fixture.paginationQuery &&
      url.searchParams.get("offset") === String(pageSize)
  )
  await assertPlainTextResults(page)
  const secondPage = await readResultSlugs(page)
  assert.ok(secondPage.length > 0, "fixture must yield a second page")
  assert.equal(
    secondPage.some((slug) => firstPage.includes(slug)),
    false,
    "pages must contain distinct results"
  )
  await pager.getByRole("link", { name: "Previous" }).click()
  await expect(page).toHaveURL(
    (url) =>
      url.pathname === searchPath &&
      url.searchParams.get("query") === fixture.paginationQuery &&
      url.searchParams.get("offset") === "0"
  )
  assert.deepEqual(await readResultSlugs(page), firstPage, "Previous restores first page")
}

test("protected hydrated search returns a prepared result and distinct pagination", async () => {
  const fixturePath = process.env.COBALT_SEARCH_FIXTURE
  const passwordPath = process.env.COBALT_LOCAL_PASSWORD_FILE
  assert.ok(
    fixturePath && passwordPath,
    "explicit search fixture and BasicAuth file required"
  )
  const fixture = await readFixture(fixturePath)
  const unauthenticated = await request.newContext()
  try {
    const denied = await unauthenticated.get(`${preview}${searchPath}`, {
      maxRedirects: 0
    })
    assert.equal(denied.status(), 401, "search route must require BasicAuth")
    assertNoindex(denied)
  } finally {
    await unauthenticated.dispose()
  }

  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      httpCredentials: {
        username: "cobalt",
        password: (await readFile(passwordPath, "utf8")).trim(),
        origin: preview
      }
    })
    try {
      const page = await context.newPage()
      /** @type {string[]} */
      const errors = []
      page.on("pageerror", (error) => errors.push(error.message))
      page.on("console", (message) => {
        if (message.type() === "error") errors.push(message.text())
      })
      const initial = await page.goto(`${preview}/${fixture.expectedSlug}`, {
        waitUntil: "networkidle"
      })
      assert.ok(initial)
      assert.equal(initial.status(), 200, "fixture page must be accessible")
      assertNoindex(initial)
      await submitHeaderSearch(page, fixture.query)
      const searchResponse = await context.request.get(page.url())
      assert.equal(searchResponse.status(), 200, "search route must be accessible")
      assertNoindex(searchResponse)
      await assertPlainTextResults(page)
      await assertExpectedResult(page, fixture)
      await assertPagination(page, fixture)
      assert.deepEqual(errors, [], "browser must have no console or page errors")
    } finally {
      await context.close()
    }
  } finally {
    await browser.close()
  }
})
