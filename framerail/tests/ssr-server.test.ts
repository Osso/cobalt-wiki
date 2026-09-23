import assert from "node:assert/strict"
import { stat, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join, relative } from "node:path"
import { test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

test("SSR servers use distinct temporary caches and remove them on close", async () => {
  const first = await createSsrServer()
  const second = await createSsrServer()
  const cacheDirs = [first.vite.config.cacheDir, second.vite.config.cacheDir]
  try {
    assert.notEqual(cacheDirs[0], cacheDirs[1])
    for (const cacheDir of cacheDirs) {
      assert.equal(relative(tmpdir(), cacheDir).startsWith(".."), false)
      await writeFile(join(cacheDir, "cleanup-proof"), "temporary")
    }
  } finally {
    await Promise.all([first.close(), second.close()])
  }
  for (const cacheDir of cacheDirs) {
    await assert.rejects(stat(cacheDir), { code: "ENOENT" })
  }
})
