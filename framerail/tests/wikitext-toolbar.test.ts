import assert from "node:assert/strict"
import { test } from "node:test"
import { applyWikitextToolbar } from "../src/lib/wikitext-toolbar.ts"

type Fixture = [string, string, number, number, string, number, number]

const inline: Fixture[] = [
  ["bold", "  hello  ", 0, 9, "  **hello**  ", 11, 11],
  ["italic", "a", 0, 1, "//a//", 5, 5],
  ["underline", "a", 0, 1, "__a__", 5, 5],
  ["strikethrough", "a", 0, 1, "--a--", 5, 5],
  ["teletype", "a", 0, 1, "{{a}}", 5, 5],
  ["superscript", "a", 0, 1, "^^a^^", 5, 5],
  ["subscript", "a", 0, 1, ",,a,,", 5, 5],
  ["raw", "a", 0, 1, "@@a@@", 5, 5],
  ["uri", "label", 0, 5, "[https://www.example.com label]", 31, 31],
  ["pageLink", "home", 0, 4, "[[[home]]]", 10, 10],
  ["image", "logo.png", 0, 8, "[[image logo.png]]", 18, 18],
  ["footnote", "note", 0, 4, "[[footnote]] note [[/footnote]]", 31, 31],
  ["inlineMath", "x+y", 0, 3, "[[$ x+y $]]", 11, 11],
  ["bibliographycitation", "ref", 0, 3, "[((bibcite ref))]", 17, 17]
]

for (const [
  control,
  value,
  start,
  end,
  expected,
  selectionStart,
  selectionEnd
] of inline) {
  test(`${control} wraps selected content`, () => {
    assert.deepEqual(applyWikitextToolbar(value, start, end, control), {
      value: expected,
      start: selectionStart,
      end: selectionEnd
    })
  })
}

test("collapsed selection inserts placeholder and selects it", () => {
  assert.deepEqual(applyWikitextToolbar("before after", 7, 7, "bold"), {
    value: "before **bold text**after",
    start: 9,
    end: 18
  })
})

test("heading joins paragraphs with blank lines and trims selection edges", () => {
  assert.deepEqual(applyWikitextToolbar("intro \n  title  \noutro", 9, 16, "heading3"), {
    value: "intro\n\n+++ title\n\noutro",
    start: 16,
    end: 16
  })
})

const blocks: [string, string, string][] = [
  ["quote", "one\ntwo", "\n> one\n> two"],
  ["numberedList", "one\ntwo", "\n# one\n# two"],
  ["bulletedList", "one\ntwo", "\n* one\n* two"],
  ["definitionList", "term", "\n: term : definition"],
  ["div", "content", "[[div]]\ncontent\n[[/div]]"],
  ["code", "print(1)", "[[code]]\nprint(1)\n[[/code]]"],
  ["html", "<p>x</p>", "[[html]]\n<p>x</p>\n[[/html]]"],
  ["math", "x+y", "[[math]]\nx+y\n[[/math]]"],
  [
    "bibliography",
    "ref",
    "[[bibliography]]\n: ref : full source reference\n[[/bibliography]]"
  ]
]
for (const [control, input, output] of blocks) {
  test(`${control} transforms selected lines`, () => {
    const result = applyWikitextToolbar(input, 0, input.length, control)
    assert.equal(result.value, output)
    assert.deepEqual([result.start, result.end], [output.length, output.length])
  })
}

for (const [control, output] of [
  ["hr", "------"],
  ["clearFloat", "~~~~"],
  ["clearFloatLeft", "~~~~<"],
  ["clearFloatRight", "~~~~>"],
  ["toc", "\n[[toc]]"]
] as [string, string][]) {
  test(`${control} inserts standalone syntax at cursor`, () => {
    assert.deepEqual(applyWikitextToolbar("", 0, 0, control), {
      value: output,
      start: output.length,
      end: output.length
    })
  })
}

test("block control separates surrounding paragraphs without duplicate newlines", () => {
  assert.equal(
    applyWikitextToolbar("before\n\nafter", 8, 8, "hr").value,
    "before\n\n------\n\nafter"
  )
  assert.equal(
    applyWikitextToolbar("before\nafter", 7, 7, "toc").value,
    "before\n[[toc]]\nafter"
  )
})

test("list indent modifies preceding line at a collapsed cursor", () => {
  assert.equal(
    applyWikitextToolbar("\n* first\n* second", 17, 17, "increaseListIndent").value,
    "\n* first\n * second"
  )
  assert.equal(
    applyWikitextToolbar("\n* first\n * second", 18, 18, "decreaseListIndent").value,
    "\n* first\n* second"
  )
})

test("indent at the first item or already indented after a parent preserves text", () => {
  assert.equal(
    applyWikitextToolbar("* first", 7, 7, "increaseListIndent").value,
    "* first"
  )
  assert.equal(
    applyWikitextToolbar("\n* first\n * second", 18, 18, "increaseListIndent").value,
    "\n* first\n * second"
  )
})
