import assert from "node:assert/strict"
import { test } from "node:test"
import { readFile, writeFile, unlink } from "node:fs/promises"
import { compile } from "svelte/compiler"
import { render } from "svelte/server"
import { createDraft } from "../src/lib/form-editor.ts"

test("source-defined fields render accessible typed controls and readonly static text", async () => {
  const source = await readFile(
    new URL("../src/lib/component/DataFormFields.svelte", import.meta.url),
    "utf8"
  )
  const compiled = compile(source, {
    generate: "server",
    filename: "DataFormFields.svelte"
  })
  assert.deepEqual(compiled.warnings, [])
  const fixture = new URL(`./.form-fields-${process.pid}.mjs`, import.meta.url)
  await writeFile(
    fixture,
    compiled.js.code.replaceAll('"../form-editor"', '"../src/lib/form-editor.ts"')
  )
  try {
    const { default: Component } = await import(fixture.href)
    const form = {
      schema: {
        properties: {},
        fields: [
          {
            name: "intro",
            kind: "static",
            properties: { value: "Read only <safe>" },
            options: []
          },
          {
            name: "name",
            kind: "text",
            properties: { label: "Name", hint: "Use a name", width: 40 },
            options: []
          },
          {
            name: "bio",
            kind: "wiki",
            properties: { label: "Biography", default: "**hello**", height: 8 },
            options: []
          },
          {
            name: "rank",
            kind: "select",
            properties: { label: "Rank" },
            options: [
              { code: 1, label: "One" },
              { code: "1", label: "String one" },
              { code: null, label: "Null" },
              { code: "null", label: "String null" }
            ]
          }
        ]
      },
      values: { name: 7, rank: "1", unknown: true }
    }
    const { body } = render(Component, { props: { form, draft: createDraft(form) } })
    assert.match(body, /Read only &lt;safe(?:&gt;|>)/)
    assert.match(body, /<label for="data-form-field-1">Name<\/label>/)
    assert.match(body, /<input[^>]*type="text"/)
    assert.match(body, /<input[^>]*size="40"/)
    assert.match(body, /<input[^>]*value="7"/)
    assert.match(body, /aria-describedby="data-form-field-1-hint"/)
    assert.match(body, /<textarea[^>]*rows="8"[^>]*>\*\*hello\*\*<\/textarea>/)
    assert.match(body, /<option[^>]*selected[^>]*>String one<\/option>/)
    assert.doesNotMatch(body, /unknown|name="wikitext"/)
    const nullable = { ...form, values: { ...form.values, rank: null } }
    const nullBody = render(Component, {
      props: { form: nullable, draft: createDraft(nullable) }
    }).body
    assert.match(nullBody, /<option[^>]*selected[^>]*>Null<\/option>/)
    assert.doesNotMatch(nullBody, /<option[^>]*selected[^>]*>String null<\/option>/)
  } finally {
    await unlink(fixture)
  }
})
