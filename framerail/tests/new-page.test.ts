import assert from "node:assert/strict"
import { test } from "node:test"
import { newPageTarget, toUnixName } from "../src/lib/new-page.ts"

// The Cobalt site's writing format, from its new-writing page.
const WRITING_FORMAT = "/^\\([\\d]{4}-(0[1-9]|1[012])-(0[1-9]|[12][0-9]|3[01])\\) /"

test("writing names in the required format open the editor with the title", () => {
  assert.deepEqual(
    newPageTarget("(2024-03-13) Atley in a Nutshell", "writing", WRITING_FORMAT),
    {
      unixName: "writing:2024-03-13-atley-in-a-nutshell",
      path: "/writing:2024-03-13-atley-in-a-nutshell/edit/true/title/(2024-03-13)%20Atley%20in%20a%20Nutshell"
    }
  )
})

test("names outside the required format are refused", () => {
  assert.deepEqual(newPageTarget("Atley in a Nutshell", "writing", WRITING_FORMAT), {
    error: "The page name is not in the required format."
  })
})

test("empty names are refused", () => {
  assert.deepEqual(newPageTarget("   ", "character", ""), {
    error: "You should provide a page name."
  })
})

test("names without a category stay in the default category", () => {
  assert.deepEqual(newPageTarget("Fan Art", "", ""), {
    unixName: "fan-art",
    path: "/fan-art/edit/true/title/Fan%20Art"
  })
})

test("unix names follow Wikidot's rules", () => {
  assert.equal(toUnixName("character:Élodie O'Brien"), "character:elodie-o-brien")
  assert.equal(toUnixName("_template"), "_template")
  assert.equal(toUnixName("Arc:  -- The Fall --"), "arc:the-fall")
  assert.equal(toUnixName("snake_case name"), "snake-case-name")
})
