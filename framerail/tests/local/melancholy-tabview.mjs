import assert from "node:assert/strict"
import { test } from "node:test"
import { chromium, expect, request } from "@playwright/test"

const url = "http://127.0.0.1:3090/character:melancholy"
const backend = "http://127.0.0.1:2749/jsonrpc"
const relationships = [
  ["Family, Friends, and Significant Other", "Bryn Baird"],
  ["Teachers and Instructors", "Lena"],
  [
    "Famous, Heroic, and Other Acquaintances, Possibly Friends Eventually",
    "Admiral/Captain Siamus Fallon"
  ],
  ["Important, But Practically Strangers, Maybe Friends Someday", "Silvestre"],
  ["People Who Might Be Friends With Other People", "Sir Kyris Lysander"]
]

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
        route: { slug: "character:melancholy", extra: "" }
      }
    }
  })
  assert.equal(response.status(), 200, "page_view must succeed")
  const payload = await response.json()
  assert.ok(!payload.error, `page_view failed: ${JSON.stringify(payload.error)}`)
  assert.equal(payload.result?.type, "found", "Melancholy page must exist")
  return {
    revision: payload.result.data.page_revision,
    source: payload.result.data.wikitext
  }
}

test("Melancholy Relationships tabs render separate styled controls and switch panels read-only", async () => {
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
    const tabs = page.locator("#page-content wj-tabs").filter({
      has: page.getByRole("tab", {
        name: relationships[0][0],
        exact: true
      })
    })
    await expect(tabs).toHaveCount(1)
    const buttons = tabs.getByRole("tab")
    await expect(buttons).toHaveCount(relationships.length)

    await assertTabAppearance(buttons)

    for (const [label, marker] of relationships) {
      const tab = tabs.getByRole("tab", { name: label, exact: true })
      const panelId = await tab.getAttribute("aria-controls")
      assert.ok(panelId, `${label} must identify a panel`)
      const panel = tabs.locator(`[role="tabpanel"][id="${panelId}"]`)
      await tab.click()
      await expect(tab).toHaveAttribute("aria-selected", "true")
      await expect(panel).toBeVisible()
      await expect(panel).toContainText(marker)
      await expect(
        tabs.getByRole("tabpanel", { includeHidden: true }).filter({ visible: true })
      ).toHaveCount(1)
    }
    assert.equal(page.url(), url, "tab selection must not navigate")
    assert.deepEqual(writes, [], "tab selection must not issue write requests")
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

/** @param {import("@playwright/test").Locator} buttons */
async function assertTabAppearance(buttons) {
  const appearance = await buttons.evaluateAll((elements) =>
    elements.map((button) => {
      const style = getComputedStyle(button)
      const box = button.getBoundingClientRect()
      return {
        label: button.textContent.trim(),
        selected: button.getAttribute("aria-selected") === "true",
        border: parseFloat(style.borderTopWidth),
        padding: parseFloat(style.paddingLeft),
        background: style.backgroundColor,
        color: style.color,
        box: { left: box.left, right: box.right, top: box.top, bottom: box.bottom }
      }
    })
  )
  assert.deepEqual(
    appearance.map(({ label }) => label),
    relationships.map(([label]) => label)
  )
  for (const { label, border, padding, box } of appearance) {
    assert.ok(border >= 1, `${label} needs a visible border`)
    assert.ok(padding >= 4, `${label} needs horizontal padding`)
    assert.ok(box.right > box.left && box.bottom > box.top, `${label} needs a box`)
  }
  for (let i = 0; i < appearance.length; i++) {
    for (let j = i + 1; j < appearance.length; j++) {
      const a = appearance[i].box
      const b = appearance[j].box
      const overlaps =
        a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
      assert.ok(!overlaps, `${appearance[i].label} overlaps ${appearance[j].label}`)
    }
  }
  const selected = appearance.find(({ selected }) => selected)
  const unselected = appearance.find(({ selected }) => !selected)
  assert.ok(selected && unselected, "one tab must initially be selected")
  assert.ok(
    selected.background !== unselected.background || selected.color !== unselected.color,
    "selected tab must be visually distinct"
  )
}
