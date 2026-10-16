import assert from "node:assert/strict"
import { test } from "node:test"
import { parseAcceptLangHeader } from "../src/lib/locales.ts"

function locales(language?: string) {
  const headers = language === undefined ? {} : { "Accept-Language": language }
  return parseAcceptLangHeader(new Request("https://example.test", { headers }))
}

test("browser measurement-unit extension preserves its base language", () => {
  assert.deepEqual(locales("en-US-u-mu-celsius"), ["en-US"])
})

test("language priorities retain scripts, regions and variants", () => {
  assert.deepEqual(
    locales("fr;q=0.8,zh-Hant-TW;q=1,en-US-u-mu-celsius;q=0.9,sl-rozaj;q=0.7"),
    ["zh-Hant-TW", "en-US", "fr", "sl-rozaj"]
  )
})

test("absent and wildcard headers supply no explicit language", () => {
  assert.deepEqual(locales(), [])
  assert.deepEqual(locales("*"), [])
})

test("invalid ranges and rejected languages cannot become backend locales", () => {
  assert.deepEqual(locales("en_US,fr;q=0,de;q=bad,en;q=0.8,*;q=0.5"), ["en"])
})
