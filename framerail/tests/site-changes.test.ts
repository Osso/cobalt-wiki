import assert from "node:assert/strict"
import { test } from "node:test"
import {
  clickSiteChanges,
  enhanceSiteChangeDates,
  formatSiteChangeDate,
  relativeSiteChangeDate,
  readSiteChangesForm,
  siteChangesPath
} from "../src/lib/site-changes.ts"

test("default choices open the first page", () => {
  assert.equal(
    siteChangesPath("system:recent-changes", { types: "", category: "", perPage: 20 }),
    "/system:recent-changes/p/1"
  )
})

test("choices become URL arguments Deepwell renders", () => {
  assert.equal(
    siteChangesPath("system:recent-changes", {
      types: "NS",
      category: "writing",
      perPage: 50
    }),
    "/system:recent-changes/p/1/perpage/50/category/writing/types/NS"
  )
})

test("all available page sizes encode except default 20", () => {
  for (const size of [10, 20, 50, 100, 200]) {
    assert.equal(
      siteChangesPath("system:recent-changes", {
        types: "",
        category: "",
        perPage: size
      }),
      `/system:recent-changes/p/1${size === 20 ? "" : `/perpage/${size}`}`
    )
  }
})

test("ALL overrides selected flags; otherwise selected flags retain NSTRAMF order", () => {
  const flags = ["N", "S", "T", "R", "A", "M", "F"]
  const all = { checked: true }
  const box = {
    querySelector(selector: string) {
      if (selector === "#rev-type-all") return all
      if (selector === "#rev-category") return { value: "story & art" }
      if (selector === "#rev-perpage") return { value: "100" }
      return null
    },
    querySelectorAll() {
      return flags.map((flag) => ({ dataset: { flag } }))
    }
  } as unknown as Element
  assert.deepEqual(readSiteChangesForm(box), {
    types: "",
    category: "story & art",
    perPage: 100
  })
  all.checked = false
  assert.deepEqual(readSiteChangesForm(box), {
    types: "NSTRAMF",
    category: "story & art",
    perPage: 100
  })
  assert.equal(
    siteChangesPath("system:recent-changes", readSiteChangesForm(box)),
    "/system:recent-changes/p/1/perpage/100/category/story%20%26%20art/types/NSTRAMF"
  )
})

test("Update button navigates from any current page back to page 1", () => {
  const previousInput = globalThis.HTMLInputElement
  const previousWindow = globalThis.window
  class Button {
    type = "button"
    closest() {
      return {
        querySelector(selector: string) {
          return selector === "#rev-type-all"
            ? { checked: true }
            : { value: selector === "#rev-perpage" ? "50" : "" }
        },
        querySelectorAll() {
          return []
        }
      }
    }
  }
  globalThis.HTMLInputElement = Button as unknown as typeof HTMLInputElement
  globalThis.window = { location: { pathname: "/system:recent-changes/p/8" } } as Window &
    typeof globalThis
  try {
    let prevented = false
    let destination = ""
    clickSiteChanges(
      {
        target: new Button(),
        preventDefault: () => (prevented = true)
      } as unknown as MouseEvent,
      (path) => (destination = path)
    )
    assert.equal(prevented, true)
    assert.equal(destination, "/system:recent-changes/p/1/perpage/50")
  } finally {
    globalThis.HTMLInputElement = previousInput
    globalThis.window = previousWindow
  }
})

const DST_START = 1741507200 // 2025-03-09T08:00:00Z, Chicago spring transition
const DST_END = 1762066800 // 2025-11-02T07:00:00Z, Chicago fall transition

test("local date includes seconds and follows Chicago DST offsets", () => {
  assert.equal(
    formatSiteChangeDate(DST_START, "America/Chicago"),
    "9 Mar 2025 - 03:00:00"
  )
  assert.equal(formatSiteChangeDate(DST_END, "America/Chicago"), "2 Nov 2025 - 01:00:00")
  assert.equal(formatSiteChangeDate(DST_START, "UTC"), "9 Mar 2025 - 08:00:00")
})

test("relative time floors each threshold and clamps zero to one second", () => {
  const now = 2_000_000
  for (const [elapsed, expected] of [
    [0, "1 second ago"],
    [59, "59 seconds ago"],
    [60, "1 minute ago"],
    [3599, "59 minutes ago"],
    [3600, "1 hour ago"],
    [86399, "23 hours ago"],
    [86400, "1 day ago"],
    [172800, "2 days ago"]
  ] as const) {
    assert.equal(relativeSiteChangeDate(now - elapsed, now), expected)
  }
})

test("enhancement replaces visible UTC text and updates native title on hover and focus", () => {
  const handlers = new Map<string, () => void>()
  const attributes = new Map([["data-timestamp", String(DST_START)]])
  const time = {
    textContent: "9 Mar 2025 - 08:00:00 UTC",
    getAttribute: (name: string) => attributes.get(name) ?? null,
    setAttribute: (name: string, value: string) => attributes.set(name, value),
    addEventListener: (name: string, listener: () => void) => handlers.set(name, listener)
  }
  enhanceSiteChangeDates(
    { querySelectorAll: () => [time] } as unknown as ParentNode,
    () => DST_START + 61,
    "America/Chicago"
  )
  assert.equal(time.textContent, "9 Mar 2025 - 03:00:00")
  handlers.get("mouseenter")?.()
  assert.equal(attributes.get("title"), "1 minute ago")
  handlers.get("focus")?.()
  assert.equal(attributes.get("title"), "1 minute ago")
})

test("invalid renderer timestamps fail instead of blanking UTC text", () => {
  const time = {
    textContent: "Date UTC",
    getAttribute: () => "not-a-timestamp"
  }
  assert.throws(
    () =>
      enhanceSiteChangeDates({ querySelectorAll: () => [time] } as unknown as ParentNode),
    /Invalid SiteChanges timestamp/
  )
  assert.equal(time.textContent, "Date UTC")
})
