import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { readFile, open } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000
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
const digest = (value) => createHash("sha256").update(value).digest("hex")

/** @param {import("@playwright/test").APIRequestContext} request */
async function rpc(request, method, params, token) {
  const response = await request.post(backend, {
    headers: {
      "X-Deepwell-Site-Id": String(siteId),
      "X-Deepwell-Session-Token": token
    },
    data: { jsonrpc: "2.0", id: 1, method, params }
  })
  assert.equal(response.status(), 200, `${method} HTTP status`)
  const payload = await response.json()
  assert.ok(!payload.error, `${method} RPC code ${payload.error?.code ?? "?"}`)
  return payload.result
}

/** @param {import("@playwright/test").BrowserContext} context */
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
        (url.pathname === "/" &&
          ["?/historyList", "?/historyRevision", "?/historyCompare"].includes(
            url.search
          )))
    if (allowed) return route.continue()
    blocked.push(`${url.origin}${url.pathname}${url.search}`)
    return route.abort()
  })
  return blocked
}

/** @param {import("@playwright/test").APIRequestContext} request */
async function readHomepage(request, token, slug) {
  const result = await rpc(
    request,
    "page_view",
    { site_id: siteId, locales: ["en"], session_token: token, route: null },
    token
  )
  assert.equal(result?.type, "found", "root homepage must exist")
  assert.equal(result.data.page.slug, slug, "root homepage must match requested slug")
  assert.ok(Number.isSafeInteger(result.data.page.page_id))
  return result.data.page.page_id
}

function filtersFor(name) {
  return Object.fromEntries(["all", ...filterNames].map((key) => [key, key === name]))
}

/** @param {import("@playwright/test").APIRequestContext} request */
async function readList(request, token, pageId, originName, filter, size, page = 1) {
  return rpc(
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
}

/** @param {import("@playwright/test").Locator} history */
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
    await expect(cells.nth(0)).toHaveText(`${expected.number}.`)
    await expect(cells.nth(2)).toHaveText(expected.flags.join(" ") || "—")
    await expect(cells.nth(6)).toHaveText(expected.comments)
    await expect(cells.nth(5).locator("time")).toHaveAttribute(
      "datetime",
      expected.created_at
    )
  }
  assert.equal(new Set(listing.rows.map((row) => row.id)).size, listing.rows.length)
}

/** @param {import("@playwright/test").Locator} history */
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

/** @param {import("@playwright/test").Locator} history */
async function checkPagination(history, request, token, pageId, originName) {
  const first = await refreshList(history, request, token, pageId, originName, "all", 10)
  assert.ok(first.total > 10, "second page requires more than ten revisions")
  await history.getByRole("button", { name: "Go to page 2" }).click()
  const second = await readList(request, token, pageId, originName, "all", 10, 2)
  await assertListing(history, second)
  const firstIds = new Set(first.rows.map((row) => row.id))
  assert.ok(second.rows.length > 0, "second page must contain rows")
  assert.ok(
    second.rows.every((row) => !firstIds.has(row.id)),
    "pages must have disjoint revision IDs"
  )
}

/** @param {import("@playwright/test").Locator} history */
async function checkDetails(history, request, token, pageId, listing, originName) {
  const first = listing.rows[0]
  const second = listing.rows[1]
  assert.ok(first && second, "two revisions required for comparison")
  const params = { site_id: siteId, page_id: pageId, origin: originName }
  await history
    .getByRole("button", { name: `View source of revision ${first.number}` })
    .click()
  const source = await rpc(
    request,
    "page_history_revision",
    { ...params, number: first.number, rendered: false },
    token
  )
  assert.equal(source.number, first.number)
  const textarea = history.locator("#history-source")
  await expect(textarea).toHaveAttribute("readonly", "")
  assert.equal(
    digest(await textarea.inputValue()),
    digest(source.source),
    "source text hash"
  )
  await expect(
    history.getByRole("region", { name: `Revision ${first.number} detail` })
  ).toBeVisible()

  await history
    .getByRole("button", { name: `View revision ${first.number}`, exact: true })
    .click()
  const rendered = await rpc(
    request,
    "page_history_revision",
    { ...params, number: first.number, rendered: true },
    token
  )
  const detail = history.getByRole("region", { name: `Revision ${first.number} detail` })
  await expect(detail).toContainText(
    "Read-only preview using current rendering context; not an original compiled snapshot."
  )
  await expect(detail.locator(".history-preview")).toBeVisible()
  assert.ok(rendered.rendered_html !== null, "rendered revision must have HTML")
  const previewHash = await detail.locator(".history-preview").evaluate((element) => {
    const normalized = document.createElement("div")
    normalized.innerHTML = element.innerHTML
    return normalized.innerHTML
  })
  const expectedHash = await detail.locator(".history-preview").evaluate((_, html) => {
    const normalized = document.createElement("div")
    normalized.innerHTML = html
    return normalized.innerHTML
  }, rendered.rendered_html)
  assert.equal(digest(previewHash), digest(expectedHash), "rendered preview hash")
  if (originName === "wikidot")
    await expect(history).toContainText(
      "Captured Wikidot source; historical whitespace may differ."
    )

  await history
    .getByRole("radio", { name: `Compare from revision ${second.number}` })
    .check()
  await history
    .getByRole("radio", { name: `Compare to revision ${first.number}` })
    .check()
  await history.getByRole("button", { name: "Compare versions" }).click()
  const from = Math.min(first.number, second.number)
  const to = Math.max(first.number, second.number)
  const comparison = await rpc(
    request,
    "page_history_compare",
    { ...params, from, to },
    token
  )
  const detailComparison = history.getByRole("region", { name: "Revision comparison" })
  await expect(detailComparison.locator("h2")).toHaveText(
    `Compare revisions ${from} to ${to}`
  )
  const lines = detailComparison.locator(".history-diff > span")
  await expect(lines).toHaveCount(comparison.lines.length)
  for (const [index, line] of comparison.lines.entries()) {
    await expect(lines.nth(index)).toHaveClass(line.kind)
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

test("local homepage History reads real paged source and native revisions", async () => {
  const fixturePath = process.env.COBALT_PAGE_PREVIEW_FIXTURE
  const passwordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  assert.ok(
    fixturePath && passwordPath && errorFile,
    "protected fixture, password and private error paths required"
  )
  let stage = "fixture"
  let browser
  try {
    const fixture = JSON.parse(await readFile(fixturePath, "utf8"))
    assert.equal(fixture.sacrificial, true)
    assert.equal(fixture.siteId, siteId)
    assert.equal(fixture.siteSlug, "cobalt-company")
    assert.equal(fixture.databaseLabel, "cobalt_local_full")
    assert.equal(fixture.homeSlug ?? "home:start", "home:start")
    assert.ok(typeof fixture.username === "string" && fixture.username.length > 0)
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
      const pageId = await readHomepage(
        context.request,
        token,
        fixture.homeSlug ?? "home:start"
      )
      const response = await page.goto(origin, { waitUntil: "networkidle" })
      assert.equal(response?.status(), 200, "root homepage HTTP status")
      await page.locator("#history-button").click()
      const history = page.getByRole("region", { name: "Page history" })
      await expect(history).toBeVisible()
      const imported = await readList(
        context.request,
        token,
        pageId,
        "wikidot",
        "all",
        20
      )
      assert.equal(imported.total, 240, "homepage imported revision count")
      const native = await readList(context.request, token, pageId, "local", "all", 20)
      assert.ok(native.total > 0, "homepage native revisions required")
      await assertListing(history, imported)
      stage = "filters and page sizes"
      for (const originName of ["wikidot", "local"]) {
        await history.locator("#history-dataset").selectOption(originName)
        const initial = originName === "wikidot" ? imported : native
        await assertListing(history, initial)
        const filtered = new Map()
        for (const filter of ["all", ...filterNames]) {
          filtered.set(
            filter,
            await refreshList(
              history,
              context.request,
              token,
              pageId,
              originName,
              filter,
              20
            )
          )
        }
        if (originName === "wikidot") {
          assert.ok(filtered.get("tags").total > 0, "homepage must have A tag evidence")
          assert.ok(
            filtered.get("meta").total > 0,
            "homepage must have M metadata evidence"
          )
          assert.ok(
            filtered.get("tags").rows.some((row) => !row.flags.includes("M")) ||
              filtered.get("meta").rows.some((row) => !row.flags.includes("A")),
            "tag and metadata selection must include distinct flag evidence"
          )
        }
        for (const size of pageSizes) {
          await refreshList(
            history,
            context.request,
            token,
            pageId,
            originName,
            "all",
            size
          )
        }
        if (originName === "wikidot") {
          stage = "second page and revision details"
          await checkPagination(history, context.request, token, pageId, originName)
          const selected = await refreshList(
            history,
            context.request,
            token,
            pageId,
            originName,
            "all",
            20
          )
          await checkDetails(
            history,
            context.request,
            token,
            pageId,
            selected,
            originName
          )
          stage = "filters and page sizes"
        }
      }
      assert.deepEqual(blocked, [], "browser must not attempt writes outside login")
      console.log(`history rows: imported=${imported.total} native=${native.total}`)
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
    throw new Error(
      `History regression failed at ${stage}; details in private error file`
    )
  } finally {
    await browser?.close()
  }
})
