import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

/** @param {string} value */
const digest = (value) => createHash("sha256").update(value).digest("hex")

/**
 * @param {import("@playwright/test").Locator} row
 * @param {{
 *   source_revision_id: number
 *   source_author_id: number | null
 *   source_flags: string[]
 *   source_created_at: string
 *   source_comments: string
 * }} revision
 */
async function assertRevisionMetadata(row, revision) {
  const cells = row.locator("td")
  assert.equal(
    (await cells.nth(1).locator("small").innerText()).trim(),
    `ID ${revision.source_revision_id}`
  )
  assert.equal((await cells.nth(2).innerText()).trim(), revision.source_flags.join(", "))
  const author =
    revision.source_author_id === null
      ? "Unknown"
      : `Wikidot ID ${revision.source_author_id}`
  assert.equal((await cells.nth(3).innerText()).trim(), author)
  const displayedDate = await cells.nth(4).locator("time").getAttribute("datetime")
  assert.ok(displayedDate, "source timestamp required")
  const expectedInstant = Date.parse(revision.source_created_at)
  assert.ok(Number.isFinite(expectedInstant), "fixture timestamp must be valid")
  assert.equal(Date.parse(displayedDate), expectedInstant)
  assert.equal((await cells.nth(5).innerText()).trim(), revision.source_comments.trim())
}

test("local imported history paginates and exposes preserved source without rollback", async () => {
  const passwordFile = process.env.COBALT_LOCAL_PASSWORD_FILE
  const payloadFile = process.env.COBALT_HISTORY_PILOT
  assert.ok(passwordFile && payloadFile, "protected local fixture paths required")
  /**
   * @type {{
   *   revisions: {
   *     source_revision_number: number
   *     source_revision_id: number
   *     source_author_id: number | null
   *     source_flags: string[]
   *     source_created_at: string
   *     source_comments: string
   *     representation: string
   *     wikitext: string
   *   }[]
   * }}
   */
  const payload = JSON.parse(await readFile(payloadFile, "utf8"))
  const expected = payload.revisions.toSorted(
    (left, right) => right.source_revision_number - left.source_revision_number
  )
  assert.equal(expected.length, 240, "this acceptance case requires the full pilot")
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const context = await browser.newContext({
      httpCredentials: {
        username: "cobalt",
        password: (await readFile(passwordFile, "utf8")).trim()
      }
    })
    const origin = "http://127.0.0.1:3089"
    const unexpectedPosts = []
    await context.route("**/*", async (route) => {
      const request = route.request()
      if (request.method() !== "POST") {
        await route.continue()
        return
      }
      const url = new URL(request.url())
      const isHistoryRead =
        url.origin === origin &&
        url.pathname === "/home:start" &&
        ["?/importedHistory", "?/importedRevision"].includes(url.search)
      if (isHistoryRead) {
        await route.continue()
        return
      }
      unexpectedPosts.push(`${url.origin}${url.pathname}${url.search}`)
      await route.abort()
    })
    const page = await context.newPage()
    const pageErrors = []
    page.on("pageerror", (error) => pageErrors.push(error))
    const content = page.locator("#page-content")
    const response = await page.goto(`${origin}/home:start`, { waitUntil: "networkidle" })
    assert.ok(response, "local homepage must return an HTTP response")
    assert.equal(response.status(), 200)
    assert.equal(pageErrors.length, 0, "homepage hydration must not throw")
    await expect(content).toBeVisible()
    const currentBodyHash = digest(await content.innerHTML())
    await page.locator("#history-button").click()
    const history = page.getByRole("region", { name: "Imported Wikidot history" })
    await expect(history).toBeVisible()
    const rows = history.locator("tbody tr")
    await expect(rows).toHaveCount(50)
    for (const count of [100, 150, 200, 240]) {
      await history.getByRole("button", { name: "Load older revisions" }).click()
      await expect(rows).toHaveCount(count)
    }
    await expect(
      history.getByRole("button", { name: "Load older revisions" })
    ).toHaveCount(0)
    const numbers = await rows.locator("td:first-child").allTextContents()
    assert.deepEqual(
      numbers.map((value) => Number(value.trim())),
      expected.map((revision) => revision.source_revision_number)
    )
    for (const [index, revision] of expected.entries()) {
      await assertRevisionMetadata(rows.nth(index), revision)
    }
    await expect(history.getByRole("button", { name: /rollback/i })).toHaveCount(0)
    for (const number of [239, 0]) {
      const revision = expected.find((entry) => entry.source_revision_number === number)
      assert.ok(revision, `pilot revision ${number} required`)
      await history
        .getByRole("button", { name: `View source revision ${number}`, exact: true })
        .click()
      const source = history.getByLabel("Imported revision source", { exact: true })
      await expect(source).toBeVisible()
      assert.equal(digest(await source.inputValue()), digest(revision.wikitext))
      await expect(source).toHaveAttribute("readonly", "")
      await expect(
        history.getByText(`Representation: ${revision.representation}`, { exact: true })
      ).toBeVisible()
      await expect(
        history.getByText(
          "Historical whitespace may differ from the original. Current page content is unchanged."
        )
      ).toBeVisible()
    }
    await page.reload({ waitUntil: "networkidle" })
    assert.equal(pageErrors.length, 0, "homepage hydration must not throw after reload")
    await expect(content).toBeVisible()
    assert.equal(digest(await content.innerHTML()), currentBodyHash)
    assert.deepEqual(unexpectedPosts, [], "history must not issue other POST actions")
  } finally {
    await browser.close()
  }
})
