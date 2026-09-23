import assert from "node:assert/strict"
import { test } from "node:test"
import { applyEnterAssist, shortcutControl } from "../src/lib/wikitext-keyboard.ts"
import { applyWikitextToolbar } from "../src/lib/wikitext-toolbar.ts"

const plainKey = {
  ctrlKey: false,
  altKey: false,
  metaKey: false,
  shiftKey: false
}

for (const [key, control] of [
  ["b", "bold"],
  ["i", "italic"],
  ["u", "underline"]
] as const) {
  test(`Ctrl+${key} selects ${control}`, () => {
    assert.equal(shortcutControl({ ...plainKey, key, ctrlKey: true }), control)
  })
}

test("source key construction ignores Meta and Shift flags but respects character case", () => {
  assert.equal(
    shortcutControl({ ...plainKey, key: "b", ctrlKey: true, metaKey: true }),
    "bold"
  )
  assert.equal(
    shortcutControl({ ...plainKey, key: "b", ctrlKey: true, shiftKey: true }),
    "bold"
  )
  assert.equal(
    shortcutControl({ ...plainKey, key: "B", ctrlKey: true, shiftKey: true }),
    null
  )
  assert.equal(shortcutControl({ ...plainKey, key: "b", metaKey: true }), null)
})

test("Alt excludes formatting shortcuts and unknown keys are ignored", () => {
  assert.equal(
    shortcutControl({ ...plainKey, key: "b", ctrlKey: true, altKey: true }),
    null
  )
  assert.equal(shortcutControl({ ...plainKey, key: "x", ctrlKey: true }), null)
})

test("Tab uses key-code fallback regardless of modifiers", () => {
  assert.equal(shortcutControl({ ...plainKey, key: "Tab" }), "tab")
  assert.equal(
    shortcutControl({
      ...plainKey,
      key: "Tab",
      ctrlKey: true,
      altKey: true,
      metaKey: true,
      shiftKey: true
    }),
    "tab"
  )
})

test("Tab inserts a literal tab and collapses caret after it", () => {
  assert.deepEqual(applyWikitextToolbar("beforeafter", 6, 6, "tab"), {
    value: "before\tafter",
    start: 7,
    end: 7
  })
})

test("Tab preserves selected text without inserting a tab and collapses caret", () => {
  assert.deepEqual(applyWikitextToolbar("beforeafter", 6, 11, "tab"), {
    value: "beforeafter",
    start: 11,
    end: 11
  })
})

test("composition excludes all shortcuts", () => {
  assert.equal(
    shortcutControl({ ...plainKey, key: "b", ctrlKey: true, isComposing: true }),
    null
  )
  assert.equal(shortcutControl({ ...plainKey, key: "Tab", isComposing: true }), null)
})

function assisted(value: string, caret = value.length) {
  return applyEnterAssist(value, caret)
}

for (const [input, output] of [
  ["* first\n", "* first\n* "],
  ["# first\n", "# first\n# "],
  ["heading\n* first\n", "heading\n* first\n* "],
  ["heading\n# first\n", "heading\n# first\n# "],
  ["* first\n* \n", "* first\n\n"],
  ["# first\n# \n", "# first\n\n"],
  ["intro\n: term : definition\n: \n", "intro\n: term : definition\n\n"],
  ["\n* parent\n * child\n", "\n* parent\n * child\n * "],
  ["\n# parent\n  # child\n", "\n# parent\n  # child\n  # "],
  ["\n: term : definition\n", "\n: term : definition\n: "],
  ["\n\tindented\n", "\n\tindented\n\t"],
  ["\n\t\n", "\n\n"],
  ["\n\tindented\n\t\n", "\n\tindented\n\n"]
] as const) {
  test(`Enter assistance transforms ${JSON.stringify(input)}`, () => {
    assert.deepEqual(assisted(input), {
      value: output,
      start: output.length,
      end: output.length
    })
  })
}

test("newline-prefixed rules do not activate on the first line", () => {
  for (const value of [": term : definition\n", "\tindented\n", "* parent\n * child\n"]) {
    assert.deepEqual(assisted(value), { value, start: value.length, end: value.length })
  }
})

for (const [opening, closing] of [
  ["[[code]]", "[[/code]]"],
  ["[[embedvideo]]", "[[/embedvideo]]"],
  ["[[math]]", "[[/math]]"],
  ["[[embed]]", "[[/embed]]"],
  ["[[code type=javascript]]", "[[/code]]"]
] as const) {
  test(`Enter inserts closing tag for ${opening} after caret`, () => {
    const input = `${opening}\n`
    assert.deepEqual(assisted(input), {
      value: `${opening}\n\n${closing}`,
      start: input.length,
      end: input.length
    })
  })
}

test("assistance transforms only text before caret and retains suffix", () => {
  const before = "intro\n* first\n"
  const suffix = "later\n* other"
  const result = assisted(before + suffix, before.length)
  assert.deepEqual(result, {
    value: "intro\n* first\n* later\n* other",
    start: before.length + 2,
    end: before.length + 2
  })
  const block = "[[math]]\n"
  assert.deepEqual(assisted(block + "future text", block.length), {
    value: "[[math]]\n\n[[/math]]future text",
    start: block.length,
    end: block.length
  })
})

test("unrelated lines and incomplete block openers are unchanged", () => {
  for (const value of [
    "plain text\n",
    "[[code]] trailing\n",
    "[[html]]\n",
    "* first\n\n"
  ]) {
    assert.deepEqual(assisted(value), { value, start: value.length, end: value.length })
  }
})

test("invalid caret fails explicitly", () => {
  assert.throws(() => assisted("text", -1), RangeError)
  assert.throws(() => assisted("text", 5), RangeError)
})
