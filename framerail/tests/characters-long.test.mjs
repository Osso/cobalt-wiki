import assert from "node:assert/strict"
import { test } from "node:test"
import { readFile, writeFile, unlink } from "node:fs/promises"
import { compile } from "svelte/compiler"
import { render } from "svelte/server"
import { createJiti } from "jiti"

const { createDraft } = await createJiti(import.meta.url).import("../src/lib/form-editor")

test("characters input renders all 200 names beyond width 80 without maxlength", async () => {
  const names = Array.from(
    { length: 200 },
    (_, index) => `portrait${String(index).padStart(3, "0")}`
  )
  const characters = names
    .map((name, index) => (index === 0 ? name : `${index % 2 === 0 ? ", " : " "}${name}`))
    .join("")
  assert.ok(characters.length > 1500)
  const source = await readFile(
    new URL("../src/lib/component/DataFormFields.svelte", import.meta.url),
    "utf8"
  )
  const compiled = compile(source, {
    generate: "server",
    filename: "DataFormFields.svelte"
  })
  assert.deepEqual(compiled.warnings, [])
  const fixture = new URL(`./.characters-long-${process.pid}.mjs`, import.meta.url)
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
            name: "characters",
            kind: "text",
            properties: { label: "CHARACTERS IN THIS WRITING", width: 80 },
            options: []
          }
        ]
      },
      values: { characters }
    }
    const { body } = render(Component, {
      props: {
        form,
        draft: createDraft(form),
        title: "Long writing roster",
        titleLabel: "Title"
      }
    })
    const input = /<input\b[^>]*id="data-form-field-0"[^>]*>/.exec(body)?.[0]
    assert.ok(input, "characters must render as an input")
    assert.match(input, /\btype="text"/)
    assert.match(input, /\bsize="80"/)
    assert.equal(/\bvalue="([^"]*)"/.exec(input)?.[1], characters)
    assert.doesNotMatch(input, /\bmaxlength(?:\s|=|\/?>)/i)
  } finally {
    await unlink(fixture)
  }
})
