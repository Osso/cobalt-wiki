import assert from "node:assert/strict"
import { test } from "node:test"
import { chromium, expect, request } from "@playwright/test"

const url = "http://127.0.0.1:3090/character:colson"
const backend = "http://127.0.0.1:2749/jsonrpc"

/** @param {import("@playwright/test").APIRequestContext} api */
async function readStoredPage(api) {
  const response = await api.post(backend, {
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "page_view",
      params: {
        site_id: 6000000,
        locales: ["en"],
        session_token: null,
        route: { slug: "character:colson", extra: "" }
      }
    }
  })
  assert.equal(response.status(), 200, "page_view must succeed")
  const payload = await response.json()
  assert.ok(!payload.error, `page_view failed: ${JSON.stringify(payload.error)}`)
  assert.equal(payload.result?.type, "found", "Colson page must exist")
  return {
    revision: payload.result.data.page_revision,
    source: payload.result.data.wikitext
  }
}

/**
 * @param {import("@playwright/test").Locator} tabs
 * @param {"Family" | "Friends" | "Other"} selected
 */
async function assertSelection(tabs, selected) {
  for (const [name, marker] of [
    ["Family", "Cressidha"],
    ["Friends", "Sil"],
    ["Other", "Fi"]
  ]) {
    const tab = tabs.getByRole("tab", { name, exact: true })
    const panelId = await tab.getAttribute("aria-controls")
    assert.ok(panelId, `${name} tab must identify a panel`)
    const panel = tabs.locator(`[role="tabpanel"][id="${panelId}"]`)
    await expect(panel).toContainText(marker)
    await expect(tab).toHaveAttribute("aria-selected", String(name === selected))
    if (name === selected) {
      await expect(panel).toBeVisible()
    } else {
      await expect(panel).toBeHidden()
    }
  }
}

test("Colson Relationships tabs switch content by click and keyboard without editing", async () => {
  const api = await request.newContext()
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const before = await readStoredPage(api)
    const context = await browser.newContext()
    const page = await context.newPage()
    /** @type {string[]} */
    const writes = []
    await context.route("**/*", async (route) => {
      if (route.request().method() === "POST") {
        writes.push(route.request().url())
        await route.abort()
      } else {
        await route.continue()
      }
    })
    const response = await page.goto(url, { waitUntil: "networkidle" })
    assert.equal(response?.status(), 200)
    const tabs = page.locator("#page-content #toc4 + wj-tabs")
    await expect(tabs).toBeVisible()
    await assertSelection(tabs, "Family")

    await tabs.getByRole("tab", { name: "Friends" }).click()
    await assertSelection(tabs, "Friends")
    await tabs.getByRole("tab", { name: "Other" }).click()
    await assertSelection(tabs, "Other")
    const family = tabs.getByRole("tab", { name: "Family" })
    await family.click()
    await assertSelection(tabs, "Family")

    await family.focus()
    await family.press("End")
    const other = tabs.getByRole("tab", { name: "Other" })
    await expect(other).toBeFocused()
    await assertSelection(tabs, "Family")
    await other.press("Enter")
    await assertSelection(tabs, "Other")
    await other.press("Home")
    await expect(family).toBeFocused()
    await family.press("Space")
    await assertSelection(tabs, "Family")

    assert.equal(page.url(), url, "tab switching must not navigate")
    assert.deepEqual(writes, [], "tab switching must not issue write requests")
    assert.deepEqual(
      await readStoredPage(api),
      before,
      "source and revision must be unchanged"
    )
  } finally {
    await browser.close()
    await api.dispose()
  }
})
