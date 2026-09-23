import assert from "node:assert/strict"
import { test } from "node:test"
import { siteChangesPath } from "../src/lib/site-changes.ts"

test("default choices open the first page", () => {
  assert.equal(
    siteChangesPath("system:recent-changes", { types: "", category: "", perPage: 20 }),
    "/system:recent-changes/p/1"
  )
})

test("choices become URL arguments Deepwell renders", () => {
  assert.equal(
    siteChangesPath("system:recent-changes", {
      types: "NS",
      category: "writing",
      perPage: 50
    }),
    "/system:recent-changes/p/1/perpage/50/category/writing/types/NS"
  )
})
