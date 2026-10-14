import assert from "node:assert/strict"
import { test } from "node:test"
import { readFile, writeFile, unlink } from "node:fs/promises"
import { compile } from "svelte/compiler"
import { render } from "svelte/server"
import { createJiti } from "jiti"

const { createDraft, changedFields } =
  /** @type {typeof import("../src/lib/form-editor")} */ (
    await createJiti(import.meta.url).import("../src/lib/form-editor")
  )
const warnings = [
  ["sexualContent", "sexual-content"],
  ["violence", "violence"],
  ["language", "language"],
  ["substanceAbuse", "substance-abuse"]
]

async function compileFields(fixture) {
  const source = await readFile(
    new URL("../src/lib/component/DataFormFields.svelte", import.meta.url),
    "utf8"
  )
  const compiled = compile(source, {
    generate: "server",
    filename: "DataFormFields.svelte"
  })
  assert.deepEqual(compiled.warnings, [])
  await writeFile(
    fixture,
    compiled.js.code.replaceAll('"../form-editor"', '"../src/lib/form-editor.ts"')
  )
  return (await import(fixture.href)).default
}

test("Writing warning radios select No for @@ or missing defaults and Yes only for selected codes", async () => {
  const fixture = new URL(`./.warning-fields-${process.pid}.mjs`, import.meta.url)
  try {
    const Component = await compileFields(fixture)
    for (const selected of [-1, ...Array.from({ length: 16 }, (_, index) => index)]) {
      /** @type {import("../src/lib/form-editor").PageForm} */
      const form = {
        schema: {
          properties: {},
          fields: warnings.map(([name, code]) => ({
            name,
            kind: "select",
            properties: { default: "@@", label: name },
            options: [
              { code: "@@", label: "No" },
              { code, label: "Yes" }
            ]
          }))
        },
        values:
          selected === -1
            ? {}
            : Object.fromEntries(
                warnings.map(([name, code], index) => [
                  name,
                  selected & (1 << index) ? code : "@@"
                ])
              )
      }
      const draft = createDraft(form)
      const { body } = render(Component, {
        props: { form, draft, title: "Warning fixture", titleLabel: "Title" }
      })
      const groups = [...body.matchAll(/<fieldset\b[^>]*>(.*?)<\/fieldset>/gs)]
      assert.equal(groups.length, 4)
      groups.forEach((group, index) => {
        const label = selected >= 0 && selected & (1 << index) ? "Yes" : "No"
        const checked = [
          ...group[1].matchAll(
            /<input[^>]*type="radio"[^>]*checked[^>]*\/>\s*(No|Yes)<\/label>/g
          )
        ]
        assert.equal(checked.length, 1)
        assert.equal(checked[0][1], label)
      })
      assert.deepEqual(changedFields(form, draft), {})
    }
  } finally {
    await unlink(fixture)
  }
})
