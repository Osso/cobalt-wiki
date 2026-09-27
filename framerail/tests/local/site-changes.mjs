import assert from "node:assert/strict"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const origin = "http://127.0.0.1:3090"
const path = "/system:recent-changes"
const screenshot = "/tmp/claude/cobalt-sitechanges-local.png"
const timeZone = "America/Chicago"

/** @param {import("@playwright/test").Request} request */
function isAllowedRead(request) {
  const url = new URL(request.url())
  if (url.origin === origin) return ["GET", "HEAD"].includes(request.method())
  return (
    url.origin === "https://d3g0gp89917ko0.cloudfront.net" &&
    request.method() === "GET" &&
    request.resourceType() === "stylesheet"
  )
}

/** @param {import("@playwright/test").BrowserContext} context */
async function guardReads(context) {
  /** @type {string[]} */
  const blocked = []
  await context.route("**/*", async (route) => {
    const request = route.request()
    if (isAllowedRead(request)) return route.continue()
    const url = new URL(request.url())
    blocked.push(`${request.method()} ${url.origin}${url.pathname}${url.search}`)
    return route.abort()
  })
  return blocked
}

/**
 * @param {import("@playwright/test").Page} page @param {string}
 *   destination
 */
async function navigate(page, destination) {
  const response = await page.goto(`${origin}${destination}`, {
    waitUntil: "networkidle"
  })
  assert.equal(response?.status(), 200, `GET ${destination}`)
  await expect(page.locator(".site-changes-table")).toBeVisible()
}

/** @param {import("@playwright/test").Page} page */
function rowsOn(page) {
  return page.locator(".site-changes-table tbody tr.changes-list-item")
}

/** @param {import("@playwright/test").Page} page */
async function revisionKeys(page) {
  const rows = rowsOn(page)
  const keys = await rows.evaluateAll((elements) =>
    elements.map((row) => {
      const link = row.querySelector("td.title a")
      const revision = row.querySelector("td.revision-no")
      return `${link?.getAttribute("href")} ${revision?.textContent?.trim()}`
    })
  )
  assert.equal(new Set(keys).size, keys.length, "revision keys must identify rows")
  return keys
}

/** @param {number} epoch */
function chicagoDate(epoch) {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone,
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23"
  }).formatToParts(new Date(epoch * 1000))
  /** @param {Intl.DateTimeFormatPartTypes} type */
  const part = (type) => parts.find((value) => value.type === type)?.value
  return `${part("day")} ${part("month")} ${part("year")} - ${part("hour")}:${part("minute")}:${part("second")}`
}

/** @param {string} title @param {number} epoch */
function assertRelativeAge(title, epoch) {
  const match = /^(\d+) (second|minute|hour|day)s? ago$/.exec(title)
  assert.ok(match, "relative time needs a numeric age and unit")
  /** @type {Record<string, number>} */
  const units = { second: 1, minute: 60, hour: 3600, day: 86400 }
  const unitSeconds = units[match[2]]
  assert.ok(unitSeconds, "relative unit must be supported")
  const seconds = Number(match[1]) * unitSeconds
  const elapsed = Math.max(1, Math.floor(Date.now() / 1000 - epoch))
  assert.ok(seconds <= elapsed + 2, "relative age cannot exceed elapsed time")
  assert.ok(elapsed - seconds < unitSeconds + 2, "relative age must track instant")
}

/** @param {import("@playwright/test").Page} page */
async function assertDates(page) {
  const dates = rowsOn(page).locator("td.mod-date time.site-change-date")
  assert.equal(await dates.count(), 20, "each default row needs a date")
  for (const date of await dates.all()) {
    await expect(date).toBeVisible()
    const iso = await date.getAttribute("datetime")
    const raw = await date.getAttribute("data-timestamp")
    assert.match(iso ?? "", /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:Z|\+00:00)$/)
    assert.match(raw ?? "", /^\d+$/)
    const epoch = Number(raw)
    assert.equal(Date.parse(iso), epoch * 1000, "ISO and epoch must be same instant")
    await expect(date).toHaveText(chicagoDate(epoch))
    const display = await date.evaluate((element) => getComputedStyle(element).display)
    assert.notEqual(display, "none", "date must not be hidden")
  }
  const date = dates.first()
  const epoch = Number(await date.getAttribute("data-timestamp"))
  for (const action of ["hover", "focus"]) {
    await date[action]()
    assertRelativeAge((await date.getAttribute("title")) ?? "", epoch)
  }
}

/** @param {import("@playwright/test").Page} page */
async function assertRowContent(page) {
  const rows = rowsOn(page)
  await expect(rows).toHaveCount(20)
  for (const row of await rows.all()) {
    await expect(row.locator("td")).toHaveCount(5)
    await expect(row.locator("td.title a")).toHaveAttribute("href", /^\//)
    await expect(row.locator("td.revision-no")).toHaveText(/^\((?:new|rev\. \d+)\)$/)
    const flagLabels = row.locator("td.flags .spantip")
    const flags = await flagLabels.allTextContents()
    for (const label of await flagLabels.all())
      await expect(label).toHaveAttribute("title", /\S+/)
    assert.ok(
      flags.every((flag) => /^[NSTRAMF]$/.test(flag)),
      "known flags only"
    )
    await expect(row.locator("td.mod-by")).toBeVisible()
  }
  assert.ok(
    (await rows.locator("td.flags .spantip").count()) > 0,
    "fixture requires flags"
  )
  const comments = rows.locator("td.title .comments")
  assert.ok((await comments.count()) > 0, "fixture requires comments")
  assert.ok(
    (await rows.locator("td.mod-by").allTextContents()).some((author) => author.trim())
  )
  console.log(
    JSON.stringify({ defaultRows: await rows.count(), comments: await comments.count() })
  )
}

/** @param {import("@playwright/test").Page} page @param {number} size */
async function selectGlobalSize(page, size) {
  await page.locator("#rev-type-all").check()
  await page.locator("#rev-category").selectOption("")
  await page.locator("#rev-perpage").selectOption(String(size))
  await page.locator('.site-changes-box input[value="Update list"]').click()
  const suffix = size === 20 ? "" : `/perpage/${size}`
  await expect(page).toHaveURL(`${origin}${path}/p/1${suffix}`)
  await expect(page.locator("#rev-perpage")).toHaveValue(String(size))
  const count = await rowsOn(page).count()
  assert.ok(count > 0 && count <= size, `size ${size}: 1..${size} rows required`)
  return count
}

/** @param {import("@playwright/test").Page} page */
async function assertWritingTags(page) {
  for (const row of await rowsOn(page).all()) {
    await expect(row.locator("td.title a")).toHaveAttribute("href", /^\/writing:/)
    await expect(row.locator("td.flags .spantip").filter({ hasText: /^A$/ })).toHaveCount(
      1
    )
  }
}

/** @param {import("@playwright/test").Page} page */
async function selectWritingTags(page) {
  await page.locator("#rev-type-all").uncheck()
  await page.locator("#rev-type-tags").check()
  await page.locator("#rev-category").selectOption("writing")
  await page.locator("#rev-perpage").selectOption("10")
  await page.locator('.site-changes-box input[value="Update list"]').click()
  const first = `${path}/p/1/perpage/10/category/writing/types/A`
  await expect(page).toHaveURL(`${origin}${first}`)
  await expect(rowsOn(page)).toHaveCount(10)
  return first
}

/** @param {import("@playwright/test").Page} page */
async function assertFilteredPager(page) {
  const first = await selectWritingTags(page)
  const next = page.locator('.site-changes-footer .pager a:has-text("next")')
  await expect(next).toHaveAttribute(
    "href",
    `${path}/p/2/perpage/10/category/writing/types/A`
  )
  const firstKeys = new Set(await revisionKeys(page))
  await assertWritingTags(page)
  await next.click()
  await expect(page).toHaveURL(`${origin}${path}/p/2/perpage/10/category/writing/types/A`)
  const previous = page.locator('.site-changes-footer .pager a:has-text("previous")')
  await expect(previous).toHaveAttribute("href", first)
  const secondKeys = await revisionKeys(page)
  assert.ok(secondKeys.length > 0, "fixture needs a second filtered page")
  assert.ok(
    secondKeys.every((key) => !firstKeys.has(key)),
    "pager pages must differ"
  )
  await assertWritingTags(page)
  await previous.click()
  await expect(page).toHaveURL(`${origin}${first}`)
  assert.deepEqual(await revisionKeys(page), [...firstKeys])
  console.log(
    JSON.stringify({
      filteredFirstRows: firstKeys.size,
      filteredSecondRows: secondKeys.length
    })
  )
}

/** @param {import("@playwright/test").Page} page */
async function assertDefault(page) {
  await navigate(page, path)
  await expect(page.locator(".site-changes-table thead th")).toHaveText([
    "Page",
    "Changes",
    "Revision",
    "Changed",
    "Author"
  ])
  await expect(page.locator("#rev-type-all")).toBeChecked()
  await expect(page.locator("#rev-category")).toHaveValue("")
  await expect(page.locator("#rev-perpage")).toHaveValue("20")
  await assertRowContent(page)
  await assertDates(page)
  return revisionKeys(page)
}

/**
 * @param {import("@playwright/test").Page} page @param {string[]}
 *   defaultKeys
 */
async function assertAllAndSizes(page, defaultKeys) {
  await page.locator("#rev-type-all").check()
  await page.locator("#rev-category").selectOption("")
  await page.locator('.site-changes-box input[value="Update list"]').click()
  await expect(page).toHaveURL(`${origin}${path}/p/1/perpage/10`)
  assert.deepEqual(await revisionKeys(page), defaultKeys.slice(0, 10), "ALL overrides A")
  /** @type {Record<number, number>} */
  const counts = {}
  for (const size of [10, 20, 50, 100, 200])
    counts[size] = await selectGlobalSize(page, size)
  console.log(JSON.stringify({ pageSizes: counts }))
}

/** @param {import("@playwright/test").Page} page */
async function captureDefault(page) {
  await navigate(page, path)
  await expect(rowsOn(page)).toHaveCount(20)
  const date = rowsOn(page).first().locator("time.site-change-date")
  const epoch = Number(await date.getAttribute("data-timestamp"))
  await expect(date).toHaveText(chicagoDate(epoch))
  const visibleDateDisplay = await date.evaluate(
    (element) => getComputedStyle(element).display
  )
  assert.notEqual(visibleDateDisplay, "none")
  await page.screenshot({ path: screenshot, fullPage: true })
  console.log(JSON.stringify({ screenshot, visibleDateDisplay }))
}

test("local SiteChanges table, Chicago dates, filters and pager are read-only", async () => {
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      viewport: { width: 1228, height: 900 },
      timezoneId: timeZone
    })
    try {
      const blocked = await guardReads(context)
      const page = await context.newPage()
      const defaultKeys = await assertDefault(page)
      await assertFilteredPager(page)
      await assertAllAndSizes(page, defaultKeys)
      await captureDefault(page)
      assert.deepEqual(
        blocked,
        [],
        "no mutation or unapproved external request may be attempted"
      )
    } finally {
      await context.close()
    }
  } finally {
    await browser.close()
  }
})
