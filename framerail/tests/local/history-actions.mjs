import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { readFile, open } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000
/** @typedef {import("../../src/lib/server/load/history").HistoryList} HistoryList */
/** @typedef {import("../../src/lib/server/load/history").HistoryOrigin} HistoryOrigin */
/** @typedef {import("../../src/lib/server/load/history").HistoryRevision} HistoryRevision */
/** @typedef {import("../../src/lib/server/load/history").HistoryComparison} HistoryComparison */
/** @typedef {"all" | "source" | "title" | "move" | "tags" | "meta" | "files"} Filter */

/** @type {Exclude<Filter, "all">[]} */
const filterNames = ["source", "title", "move", "tags", "meta", "files"]
const filterLabels = [
  "ALL",
  "source changes",
  "title changes",
  "page name changes",
  "tags changes",
  "metadata changes",
  "files changes"
]
/** @type {Record<Exclude<Filter, "all">, string>} */
const sourceFlags = {
  source: "S",
  title: "T",
  move: "R",
  tags: "A",
  meta: "M",
  files: "F"
}
const pageSizes = [10, 20, 50, 100, 200]
const errorFile = process.env.COBALT_HISTORY_ACTION_ERROR_FILE
/** @param {string} value */
const digest = (value) => createHash("sha256").update(value).digest("hex")

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
  /** @type {{ error?: { code: number }; result: unknown }} */
  const payload = await response.json()
  assert.ok(!payload.error, `${method} RPC code ${payload.error?.code ?? "?"}`)
  return payload.result
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {string} username
 * @param {string} password
 */
async function login(context, username, password) {
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  await expect(page.locator("#login")).toBeVisible()
  await page.locator('#login [name="nameOrEmail"]').fill(username)
  await page.locator('#login [name="password"]').fill(password)
  await page.locator('#login button[type="submit"]').click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (entry) => entry.name === "wikijump_token" && entry.domain === "127.0.0.1"
  )
  assert.ok(cookie, "login session cookie required")
  return { page, token: decodeURIComponent(cookie.value) }
}

/** @param {import("@playwright/test").BrowserContext} context */
async function guardBrowserWrites(context) {
  /** @type {string[]} */
  const blocked = []
  await context.route("**/*", async (route) => {
    const request = route.request()
    if (["GET", "HEAD", "OPTIONS"].includes(request.method())) {
      return route.continue()
    }
    const url = new URL(request.url())
    const allowed =
      request.method() === "POST" &&
      url.origin === origin &&
      ((url.pathname === "/-/login" && ["", "?/login"].includes(url.search)) ||
        (["/", "/home:start"].includes(url.pathname) &&
          ["?/historyList", "?/historyRevision", "?/historyCompare"].includes(
            url.search
          )))
    if (allowed) return route.continue()
    blocked.push(`${url.origin}${url.pathname}${url.search}`)
    return route.abort()
  })
  return blocked
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {string | null} slug
 */
async function readPage(request, token, slug) {
  const route = slug === null ? null : { slug, extra: "" }
  const result =
    /**
     * @type {{
     *   type: string
     *   data: { page: { slug: string; page_id: number } }
     * } | null}
     */ (
      await rpc(
        request,
        "page_view",
        { site_id: siteId, locales: ["en"], session_token: token, route },
        token
      )
    )
  assert.equal(result?.type, "found", `${slug ?? "root"} page must exist`)
  assert.ok(result, "page view result required")
  if (slug !== null) assert.equal(result.data.page.slug, slug)
  assert.ok(Number.isSafeInteger(result.data.page.page_id))
  return { id: result.data.page.page_id, slug: result.data.page.slug }
}

/** @param {Filter} name */
function filtersFor(name) {
  return Object.fromEntries(["all", ...filterNames].map((key) => [key, key === name]))
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {number} pageId
 * @param {HistoryOrigin} originName
 * @param {Filter} filter
 * @param {number} size
 * @param {number} [page]
 * @returns {Promise<HistoryList>}
 */
async function readList(request, token, pageId, originName, filter, size, page = 1) {
  return /** @type {Promise<HistoryList>} */ (
    rpc(
      request,
      "page_history_list",
      {
        site_id: siteId,
        page_id: pageId,
        origin: originName,
        page,
        per_page: size,
        filters: filtersFor(filter)
      },
      token
    )
  )
}

/**
 * @param {import("@playwright/test").Locator} history
 * @param {HistoryList} listing
 */
async function assertListing(history, listing) {
  await expect(history.locator(".history-table tbody tr")).toHaveCount(
    listing.rows.length
  )
  await expect(
    history.getByText(
      `${listing.total} revisions · Page ${listing.page} of ${listing.total_pages}`
    )
  ).toBeVisible()
  const rows = history.locator(".history-table tbody tr")
  for (const [index, expected] of listing.rows.entries()) {
    const cells = rows.nth(index).locator("td")
    await expect(cells.nth(0)).toHaveText(String(expected.number))
    await expect(cells.nth(2).locator("[data-flag]")).toHaveCount(expected.flags.length)
    for (const [flagIndex, flag] of expected.flags.entries()) {
      await expect(cells.nth(2).locator("[data-flag]").nth(flagIndex)).toHaveAttribute(
        "data-flag",
        flag
      )
    }
    await expect(cells.nth(6)).toHaveText(expected.comments)
    await expect(cells.nth(5).locator("time")).toHaveAttribute(
      "datetime",
      expected.created_at
    )
  }
  if (listing.rows.length === 0) {
    await expect(history.getByText("No revisions match this selection.")).toBeVisible()
  }
  assert.equal(new Set(listing.rows.map((row) => row.id)).size, listing.rows.length)
}

/**
 * @param {import("@playwright/test").Locator} history
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {number} pageId
 * @param {HistoryOrigin} originName
 * @param {Filter} filter
 * @param {number} size
 */
async function refreshList(history, request, token, pageId, originName, filter, size) {
  const checkboxes = history.locator('fieldset input[type="checkbox"]')
  for (const [index, name] of ["all", ...filterNames].entries()) {
    const checkbox = checkboxes.nth(index)
    await expect(checkbox.locator("xpath=.."), `filter ${name}`).toContainText(
      filterLabels[index]
    )
    if (name === filter) await checkbox.check()
    else await checkbox.uncheck()
  }
  await history.locator("#history-perpage").selectOption(String(size))
  await history.getByRole("button", { name: "Update list" }).click()
  const expected = await readList(request, token, pageId, originName, filter, size)
  await expect(history).toHaveAttribute("aria-busy", "false")
  await assertListing(history, expected)
  if (filter !== "all") {
    assert.ok(
      expected.rows.every((row) => row.flags.includes(sourceFlags[filter])),
      `filtered ${filter} rows must carry ${sourceFlags[filter]} flag`
    )
  }
  return expected
}

/**
 * @param {import("@playwright/test").Locator} history
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {number} pageId
 * @param {HistoryOrigin} originName
 */
async function checkPagination(history, request, token, pageId, originName) {
  const first = await refreshList(history, request, token, pageId, originName, "all", 10)
  assert.ok(first.total > 10, "second page requires more than ten revisions")
  await history.getByRole("button", { name: "Go to page 2", exact: true }).click()
  const second = await readList(request, token, pageId, originName, "all", 10, 2)
  await assertListing(history, second)
  const firstIds = new Set(first.rows.map((row) => row.id))
  assert.ok(second.rows.length > 0, "second page must contain rows")
  assert.ok(
    second.rows.every((row) => !firstIds.has(row.id)),
    "pages must have disjoint revision IDs"
  )
}

/**
 * @param {import("@playwright/test").Locator} history
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {number} pageId
 * @param {number} revisionNumber
 * @param {HistoryOrigin} originName
 */
async function checkSourceAndPreview(
  history,
  request,
  token,
  pageId,
  revisionNumber,
  originName
) {
  const params = { site_id: siteId, page_id: pageId, origin: originName }
  await history
    .getByRole("button", { name: `View source of revision ${revisionNumber}` })
    .click()
  const source = /** @type {HistoryRevision} */ (
    await rpc(
      request,
      "page_history_revision",
      { ...params, number: revisionNumber, rendered: false },
      token
    )
  )
  assert.equal(source.number, revisionNumber)
  const textarea = history.locator("#history-source")
  await expect(textarea).toHaveAttribute("readonly", "")
  assert.equal(
    digest(await textarea.inputValue()),
    digest(source.source),
    "source text hash"
  )
  await expect(
    history.getByRole("region", { name: `Revision ${revisionNumber} detail` })
  ).toBeVisible()
}

/**
 * @param {import("@playwright/test").Locator} history
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {number} pageId
 * @param {number} revisionNumber
 * @param {HistoryOrigin} originName
 */
async function checkRenderedPreview(
  history,
  request,
  token,
  pageId,
  revisionNumber,
  originName
) {
  const params = { site_id: siteId, page_id: pageId, origin: originName }
  await history
    .getByRole("button", { name: `View revision ${revisionNumber}`, exact: true })
    .click()
  const rendered = /** @type {HistoryRevision} */ (
    await rpc(
      request,
      "page_history_revision",
      { ...params, number: revisionNumber, rendered: true },
      token
    )
  )
  const detail = history.getByRole("region", {
    name: `Revision ${revisionNumber} detail`
  })
  await expect(detail).toContainText(
    "Read-only preview using current rendering context; not an original compiled snapshot."
  )
  await expect(detail.locator(".history-preview")).toBeVisible()
  assert.ok(rendered.rendered_html !== null, "rendered revision must have HTML")
  await assertRenderedHtml(detail, rendered.rendered_html)
  if (originName === "wikidot") {
    await expect(history).toContainText(
      "Captured Wikidot source; historical whitespace may differ."
    )
  }
}

/**
 * @param {import("@playwright/test").Locator} detail
 * @param {string} expectedHtml
 */
async function assertRenderedHtml(detail, expectedHtml) {
  const previewHash = await detail.locator(".history-preview").evaluate((element) => {
    const normalized = document.createElement("div")
    normalized.innerHTML = element.innerHTML
    return normalized.innerHTML
  })
  const expectedHash = await detail.locator(".history-preview").evaluate((_, html) => {
    const normalized = document.createElement("div")
    normalized.innerHTML = html
    return normalized.innerHTML
  }, expectedHtml)
  assert.equal(digest(previewHash), digest(expectedHash), "rendered preview hash")
}

/**
 * @param {import("@playwright/test").Locator} history
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {number} pageId
 * @param {number} firstNumber
 * @param {number} secondNumber
 * @param {HistoryOrigin} originName
 */
async function checkComparison(
  history,
  request,
  token,
  pageId,
  firstNumber,
  secondNumber,
  originName
) {
  const params = { site_id: siteId, page_id: pageId, origin: originName }
  await history
    .getByRole("radio", { name: `Compare from revision ${secondNumber}` })
    .check()
  await history.getByRole("radio", { name: `Compare to revision ${firstNumber}` }).check()
  await history.getByRole("button", { name: "Compare versions" }).click()
  const from = Math.min(firstNumber, secondNumber)
  const to = Math.max(firstNumber, secondNumber)
  const comparison = /** @type {HistoryComparison} */ (
    await rpc(request, "page_history_compare", { ...params, from, to }, token)
  )
  const detailComparison = history.getByRole("region", { name: "Revision comparison" })
  await expect(detailComparison.locator("h2")).toHaveText(
    `Compare revisions ${from} to ${to}`
  )
  await assertComparisonLines(detailComparison, comparison)
}

/**
 * @param {import("@playwright/test").Locator} detailComparison
 * @param {HistoryComparison} comparison
 */
async function assertComparisonLines(detailComparison, comparison) {
  const lines = detailComparison.locator(".history-diff > span")
  await expect(lines).toHaveCount(comparison.lines.length)
  for (const [index, line] of comparison.lines.entries()) {
    await expect(lines.nth(index)).toHaveClass(new RegExp(`\\b${line.kind}\\b`))
    assert.equal(await lines.nth(index).textContent(), line.text, "escaped diff text")
    if (line.kind !== "same") {
      await expect(
        lines.nth(index).locator(line.kind === "insert" ? "ins" : "del")
      ).toHaveText(line.text)
    }
  }
  assert.equal(
    digest((await lines.allTextContents()).join("")),
    digest(comparison.lines.map((line) => line.text).join("")),
    "comparison text hash"
  )
}

/**
 * @param {import("@playwright/test").Locator} history
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {number} pageId
 * @param {HistoryList} listing
 * @param {HistoryOrigin} originName
 */
async function checkDetails(history, request, token, pageId, listing, originName) {
  const first = listing.rows[0]
  const second = listing.rows[1]
  assert.ok(first && second, "two revisions required for comparison")
  await checkSourceAndPreview(history, request, token, pageId, first.number, originName)
  await checkRenderedPreview(history, request, token, pageId, first.number, originName)
  await checkComparison(
    history,
    request,
    token,
    pageId,
    first.number,
    second.number,
    originName
  )
}

/** @param {string} fixturePath */
async function readHistoryFixture(fixturePath) {
  /**
   * @type {{
   *   sacrificial: boolean
   *   siteId: number
   *   siteSlug: string
   *   databaseLabel: string
   *   homeSlug?: string
   *   username: string
   * }}
   */
  const fixture = JSON.parse(await readFile(fixturePath, "utf8"))
  assert.equal(fixture.sacrificial, true)
  assert.equal(fixture.siteId, siteId)
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.homeSlug ?? "home:_public", "home:_public")
  assert.ok(typeof fixture.username === "string" && fixture.username.length > 0)
  return fixture
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {string} homeSlug
 */
async function checkRootHomepage(page, request, token, homeSlug) {
  const root = await readPage(request, token, null)
  assert.equal(root.slug, homeSlug)
  const rootResponse = await page.goto(origin, { waitUntil: "networkidle" })
  assert.equal(rootResponse?.status(), 200, "root homepage HTTP status")
  await page.locator("#history-button").click()
  const history = page.getByRole("region", { name: "Page history" })
  await expect(history).toBeVisible()
  const rootImported = await readList(request, token, root.id, "wikidot", "all", 20)
  assert.equal(rootImported.total, 57, "root resolved homepage imported revisions")
  await assertListing(history, rootImported)
  return rootImported.total
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 */
async function checkNamedHomepage(page, request, token) {
  const named = await readPage(request, token, "home:start")
  const pageId = named.id
  const namedResponse = await page.goto(`${origin}/home:start`, {
    waitUntil: "networkidle"
  })
  assert.equal(namedResponse?.status(), 200, "named homepage HTTP status")
  await page.locator("#history-button").click()
  const history = page.getByRole("region", { name: "Page history" })
  await expect(history).toBeVisible()
  const imported = await readList(request, token, pageId, "wikidot", "all", 20)
  assert.equal(imported.total, 240, "named homepage imported revision count")
  const native = await readList(request, token, pageId, "local", "all", 20)
  assert.ok(native.total > 0, "named homepage native revisions required")
  await assertListing(history, imported)
  return { history, pageId, imported, native }
}

/** @param {Map<Filter, HistoryList>} filtered */
function assertImportedFilterCounts(filtered) {
  /** @type {Record<Filter, number>} */
  const counts = {
    all: 240,
    source: 232,
    title: 1,
    move: 5,
    tags: 1,
    meta: 0,
    files: 1
  }
  /** @type {Filter[]} */
  const filters = ["all", ...filterNames]
  for (const filter of filters) {
    assert.equal(filtered.get(filter)?.total, counts[filter], `${filter} revision count`)
  }
  assert.ok(
    filtered.get("tags")?.rows.every((row) => !row.flags.includes("M")),
    "tag evidence must not be mislabeled as metadata"
  )
}

/**
 * @param {import("@playwright/test").Locator} history
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {number} pageId
 * @param {HistoryOrigin} originName
 * @param {HistoryList} initial
 * @param {(stage: string) => void} setStage
 */
async function checkFiltersAndSizes(
  history,
  request,
  token,
  pageId,
  originName,
  initial,
  setStage
) {
  if (originName === "local") {
    await history.locator("#history-dataset").selectOption(originName)
  }
  await assertListing(history, initial)
  /** @type {Map<Filter, HistoryList>} */
  const filtered = new Map()
  /** @type {Filter[]} */
  const filters = ["all", ...filterNames]
  for (const filter of filters) {
    filtered.set(
      filter,
      await refreshList(history, request, token, pageId, originName, filter, 20)
    )
  }
  if (originName === "wikidot") assertImportedFilterCounts(filtered)
  for (const size of pageSizes) {
    await refreshList(history, request, token, pageId, originName, "all", size)
  }
  if (originName === "wikidot") {
    setStage("second page and revision details")
    await checkPagedDetails(history, request, token, pageId, originName)
    setStage("filters and page sizes")
  }
}

/**
 * @param {import("@playwright/test").Locator} history
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {number} pageId
 * @param {HistoryOrigin} originName
 */
async function checkPagedDetails(history, request, token, pageId, originName) {
  await checkPagination(history, request, token, pageId, originName)
  const selected = await refreshList(
    history,
    request,
    token,
    pageId,
    originName,
    "all",
    20
  )
  await checkDetails(history, request, token, pageId, selected, originName)
}

test("root and named homepage History read real paged source and native revisions", async () => {
  const fixturePath = process.env.COBALT_PAGE_PREVIEW_FIXTURE
  const passwordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(
    fixturePath && passwordPath && errorFile,
    "protected fixture, password and private error paths required"
  )
  let stage = "fixture"
  let browser
  try {
    const fixture = await readHistoryFixture(fixturePath)
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
      const blocked = await guardBrowserWrites(context)
      stage = "login"
      const { page, token } = await login(context, fixture.username, password)
      stage = "homepage identity"
      const rootTotal = await checkRootHomepage(
        page,
        context.request,
        token,
        fixture.homeSlug ?? "home:_public"
      )

      stage = "named homepage identity"
      const { history, pageId, imported, native } = await checkNamedHomepage(
        page,
        context.request,
        token
      )
      stage = "filters and page sizes"
      /** @type {HistoryOrigin[]} */
      const origins = ["wikidot", "local"]
      for (const originName of origins) {
        const initial = originName === "wikidot" ? imported : native
        await checkFiltersAndSizes(
          history,
          context.request,
          token,
          pageId,
          originName,
          initial,
          (nextStage) => {
            stage = nextStage
          }
        )
      }
      assert.deepEqual(blocked, [], "browser must not attempt writes outside login")
      console.log(
        `history rows: root=${rootTotal} named=${imported.total} native=${native.total}`
      )
    } finally {
      await context.close()
    }
  } catch (error) {
    const detail = `${stage}: ${error instanceof Error ? error.stack : String(error)}\n`
    const failure = await open(errorFile, "w", 0o600)
    try {
      await failure.chmod(0o600)
      await failure.writeFile(detail)
    } finally {
      await failure.close()
    }
    assert.fail(`History regression failed at ${stage}; details in private error file`)
  } finally {
    await browser?.close()
  }
})
