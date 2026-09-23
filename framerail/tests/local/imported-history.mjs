import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

/** @param {string} value */
const digest = (value) => createHash("sha256").update(value).digest("hex")

test("local imported history paginates and exposes preserved source without rollback", async () => {
  const passwordFile = process.env.COBALT_LOCAL_PASSWORD_FILE
  const payloadFile = process.env.COBALT_HISTORY_PILOT
  assert.ok(passwordFile && payloadFile, "protected local fixture paths required")
  /**
   * @type {{
   *   revisions: { source_revision_number: number; wikitext: string }[]
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
    const page = await context.newPage()
    const response = await page.goto("http://127.0.0.1:3089/home:start", {
      waitUntil: "networkidle"
    })
    assert.ok(response, "local homepage must return an HTTP response")
    assert.equal(response.status(), 200)
    await page.locator("#history-button, .button-history").click()
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
    await expect(history.getByRole("button", { name: /rollback/i })).toHaveCount(0)
    await history
      .getByRole("button", { name: "View source revision 0", exact: true })
      .click()
    const source = history.getByLabel("Imported revision source", { exact: true })
    await expect(source).toBeVisible()
    const oldest = expected.at(-1)
    assert.ok(oldest, "the pilot must include its oldest revision")
    assert.equal(digest(await source.inputValue()), digest(oldest.wikitext))
    await expect(source).toHaveAttribute("readonly", "")
    await expect(
      history.getByText(
        "Historical whitespace may differ from the original. Current page content is unchanged."
      )
    ).toBeVisible()
    await page.reload({ waitUntil: "networkidle" })
    await expect(page.locator("#page-content")).toBeVisible()
  } finally {
    await browser.close()
  }
})
