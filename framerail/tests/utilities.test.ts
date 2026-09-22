import assert from "node:assert/strict"
import { test } from "node:test"
import { createJiti } from "jiti"

const jiti = createJiti(import.meta.url)
const { PreferenceHandler } =
  await jiti.import<typeof import("../src/lib/util/pref")>("../src/lib/util/pref")
const { wjfetch, DEFAULT_TIMEOUT } =
  await jiti.import<typeof import("../src/lib/fetch")>("../src/lib/fetch")
const { parseAcceptLangHeader } =
  await jiti.import<typeof import("../src/lib/locales")>("../src/lib/locales")

test("wrapped preferences preserve null leaves and persist nested object and array edits", (t) => {
  const entries = new Map<string, string>()
  const storage: Storage = {
    get length() {
      return entries.size
    },
    clear: () => entries.clear(),
    getItem: (key) => entries.get(key) ?? null,
    key: (index) => [...entries.keys()][index] ?? null,
    removeItem: (key) => {
      entries.delete(key)
    },
    setItem: (key, value) => {
      entries.set(key, value)
    }
  }
  const previous = Object.getOwnPropertyDescriptor(globalThis, "localStorage")
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: storage
  })
  t.after(() => {
    if (previous) Object.defineProperty(globalThis, "localStorage", previous)
    else Reflect.deleteProperty(globalThis, "localStorage")
  })
  const preferences = new PreferenceHandler("test:")
  const defaults = {
    selected: null,
    nested: { enabled: true },
    rows: [{ label: "first" }, null]
  }
  const value = preferences.wrap("display", defaults)
  assert.equal(value.selected, null)
  assert.equal(value.rows[1], null)
  value.nested.enabled = false
  value.rows[0] = { label: "updated" }
  assert.deepEqual(preferences.get("display"), {
    selected: null,
    nested: { enabled: false },
    rows: [{ label: "updated" }, null]
  })
})

test("fetch forwards RequestInit and keeps caller signal precedence", async (t) => {
  const response = new Response("saved", { status: 201 })
  const requests: { input: RequestInfo | URL; init?: RequestInit }[] = []
  t.mock.method(
    globalThis,
    "fetch",
    async (input: RequestInfo | URL, init?: RequestInit) => {
      requests.push({ input, init })
      return response
    }
  )
  const timeouts: number[] = []
  t.mock.method(AbortSignal, "timeout", (milliseconds: number) => {
    timeouts.push(milliseconds)
    return new AbortController().signal
  })
  const signal = new AbortController().signal
  const url = new URL("https://example.invalid/save")
  assert.equal(
    await wjfetch(url, { method: "POST", body: "content", timeout: 250, signal }),
    response
  )
  await wjfetch(new Request(url))
  assert.deepEqual(timeouts, [250, DEFAULT_TIMEOUT])
  assert.equal(requests[0].input, url)
  assert.equal(requests[0].init?.method, "POST")
  assert.equal(requests[0].init?.body, "content")
  assert.equal(requests[0].init?.signal, signal)
  assert.equal(Object.hasOwn(requests[0].init ?? {}, "timeout"), false)
})

test("language parsing accepts actual requests and absent headers", () => {
  assert.deepEqual(parseAcceptLangHeader(new Request("https://example.invalid")), [])
  assert.deepEqual(
    parseAcceptLangHeader(
      new Request("https://example.invalid", {
        headers: { "Accept-Language": "fr-CA;q=0.8,en-US;q=1,zh-Hant-TW;q=0.5" }
      })
    ),
    ["en-US", "fr-CA", "zh-Hant-TW"]
  )
})
