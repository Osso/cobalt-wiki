import assert from "node:assert/strict"
import { test } from "node:test"
import { mergeDraftSource } from "../src/lib/form-editor.ts"

test("an unchanged restored draft keeps its exact source bytes", () => {
  const source = "# saved draft\nname: 'Draft name'\ncount: 3\nunknown: true\n"
  assert.equal(
    mergeDraftSource(source, { name: "Draft name", count: 3, unknown: true }, {}),
    source
  )
})

test("editing a restored form keeps every typed and unknown draft value", () => {
  const result = mergeDraftSource(
    "name: Draft name\ncount: 3\nunknown: true\n",
    { name: "Draft name", count: 3, unknown: true, note: null, fixed: "Read only" },
    { name: "Edited draft name" }
  )
  assert.deepEqual(JSON.parse(result), {
    name: "Edited draft name",
    count: 3,
    unknown: true,
    note: null,
    fixed: "Read only"
  })
})
