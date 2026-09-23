import assert from "node:assert/strict"
import { test } from "node:test"
import { readFile, writeFile, unlink } from "node:fs/promises"
import { compile } from "svelte/compiler"
import { render } from "svelte/server"

const component = new URL("../src/lib/component/WikitextWizard.svelte", import.meta.url)

/** @param {import("../src/lib/wikitext-wizards").WizardKind} kind */
async function renderWizard(kind, source = "", extra = {}) {
  const compiled = compile(await readFile(component, "utf8"), {
    generate: "server",
    filename: "WikitextWizard.svelte"
  })
  assert.deepEqual(compiled.warnings, [])
  const fixture = new URL(`./.wikitext-wizard-${process.pid}.mjs`, import.meta.url)
  await writeFile(
    fixture,
    compiled.js.code
      .replaceAll('"$lib/wikitext-wizards"', '"../src/lib/wikitext-wizards.ts"')
      .replaceAll('"$lib/image-check"', '"../src/lib/image-check.ts"')
  )
  try {
    const { default: Wizard } = await import(`${fixture.href}?kind=${kind}`)
    return render(Wizard, {
      props: { kind, source, onInsert() {}, onCancel() {}, ...extra }
    }).body
  } finally {
    await unlink(fixture)
  }
}

test("table and URL wizard render source defaults with labeled controls", async () => {
  const table = await renderWizard("table")
  assert.match(table, /<dialog[^>]*aria-label="Table wizard"/)
  assert.match(table, /Number of rows/)
  assert.match(table, /value="3"/)
  assert.match(table, /Number of columns/)
  assert.match(table, /First row as header/)
  const uri = await renderWizard("uri")
  assert.match(uri, /value="http:\/\/"/)
  assert.match(uri, /Anchor text/)
  assert.match(uri, /Open in a new window/)
})

test("image hides unavailable attachments and keeps source-like position choices", async () => {
  const body = await renderWizard("image")
  assert.doesNotMatch(body, /attached file/i)
  assert.match(body, /Flickr/)
  assert.match(body, /value="fl"/)
  assert.match(body, /value="fr"/)
  assert.doesNotMatch(body, /Image size/)
  const available = await renderWizard("image", "", { attachmentLookup: async () => [] })
  assert.match(available, /attached file/i)
})

test("equation source displays escaped preview and reports absent labels", async () => {
  const empty = await renderWizard("eref")
  assert.match(empty, /no labelled equations found/i)
  const body = await renderWizard("eref", "[[math eq1]]\n<b>unsafe</b>\n[[/math]]")
  assert.match(body, /eq1/)
  assert.match(body, /&lt;b(?:&gt;|>)unsafe&lt;\/b(?:&gt;|>)/)
  assert.doesNotMatch(body, /<b>unsafe<\/b>/)
  assert.match(body, /Eq\.\(/)
})
