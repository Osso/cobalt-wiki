import assert from "node:assert/strict"
import { test } from "node:test"
import { createJiti } from "jiti"
import type { PageForm } from "../src/lib/form-editor"

const { createDraft, changedFields, editContent, fieldText } = await createJiti(
  import.meta.url
).import<typeof import("../src/lib/form-editor")>("../src/lib/form-editor")

const form: PageForm = {
  schema: {
    properties: {},
    fields: [
      { name: "intro", kind: "static", properties: { value: "Read only" }, options: [] },
      { name: "name", kind: "text", properties: { label: "Name" }, options: [] },
      { name: "bio", kind: "wiki", properties: { default: "**New**" }, options: [] },
      {
        name: "rank",
        kind: "select",
        properties: {},
        options: [
          { code: 1, label: "One" },
          { code: "1", label: "String one" },
          { code: false, label: "None" }
        ]
      }
    ]
  },
  values: { name: 7, rank: 1, unknown: true }
}

test("unchanged numeric text and typed select submit no updates or unknown keys", () => {
  assert.deepEqual(changedFields(form, createDraft(form)), {})
  assert.deepEqual(form.values, { name: 7, rank: 1, unknown: true })
})

test("text/wiki edits are strings; reverting displayed numeric text is unchanged", () => {
  const draft = { ...createDraft(form), name: "Ada", bio: "[[include Box]]" }
  assert.deepEqual(changedFields(form, draft), { name: "Ada", bio: "[[include Box]]" })
  assert.deepEqual(changedFields(form, { ...createDraft(form), name: "7" }), {})
})

test("select codes retain distinct number/string/boolean types", () => {
  assert.deepEqual(changedFields(form, { ...createDraft(form), rank: "1" }), {
    rank: "1"
  })
  assert.deepEqual(changedFields(form, { ...createDraft(form), rank: false }), {
    rank: false
  })
})

test("static and unknown draft keys cannot become editable updates", () => {
  assert.deepEqual(
    changedFields(form, { ...createDraft(form), intro: "tampered", unknown: false }),
    {}
  )
  assert.equal(fieldText(form.schema.fields[0], form.values), "Read only")
})

test("defaults display without being saved and null values remain unchanged", () => {
  assert.equal(createDraft(form).bio, "**New**")
  const nullable = { ...form, values: { name: null, rank: null } }
  assert.deepEqual(changedFields(nullable, createDraft(nullable)), {})
})

test("wire payloads contain exactly one content variant", () => {
  assert.deepEqual(editContent(undefined, { rank: false }), {
    form_updates: { rank: false }
  })
  assert.deepEqual(editContent("raw wiki", undefined), { wikitext: "raw wiki" })
  assert.deepEqual(editContent(undefined, {}), { form_updates: {} })
  assert.throws(() => editContent("raw", {}), /exactly one/)
  assert.throws(() => editContent(undefined, undefined), /exactly one/)
})
