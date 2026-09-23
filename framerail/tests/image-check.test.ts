import assert from "node:assert/strict"
import { test } from "node:test"
import { imageCheckBounds } from "../src/lib/image-check.ts"

test("image check centers a loaded image with 200px padding", () => {
  assert.deepEqual(imageCheckBounds(320, 180, 1440, 900), {
    width: 520,
    height: 380,
    left: 460,
    top: 260
  })
})

test("image check caps large images at 100px below available screen dimensions", () => {
  assert.deepEqual(imageCheckBounds(2000, 1200, 1440, 900), {
    width: 1340,
    height: 800,
    left: 50,
    top: 50
  })
})
