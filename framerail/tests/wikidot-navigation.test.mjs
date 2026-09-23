import assert from "node:assert/strict"
import { readFile, writeFile, unlink } from "node:fs/promises"
import { test } from "node:test"
import { compile, preprocess } from "svelte/compiler"
import { compileString } from "sass"
import { render } from "svelte/server"

test("Wikidot layout omits disabled sidebar and retains configured navigation", async () => {
  const source = await readFile(
    new URL("../src/lib/sigma-esque/wikidot.svelte", import.meta.url),
    "utf8"
  )
  const prepared = await preprocess(source, {
    style: ({ content, attributes }) =>
      attributes.lang === "scss" ? { code: compileString(content).css } : undefined
  })
  const compiled = compile(prepared.code, {
    generate: "server",
    filename: "wikidot.svelte"
  })
  const fixture = new URL(`./.wikidot-navigation-${process.pid}.mjs`, import.meta.url)
  await writeFile(fixture, compiled.js.code)
  try {
    const { default: Component } = await import(fixture.href)
    for (const sideBarHtml of [null, ""]) {
      const { body } = render(Component, { props: { sideBarHtml } })
      assert.doesNotMatch(body, /id="side-bar"/)
      assert.match(body, /id="main-content"/)
    }
    const { body } = render(Component, {
      props: { sideBarHtml: '<a href="/reference">Reference</a>' }
    })
    assert.match(body, /<div id="side-bar"[^>]*>/)
    assert.match(body, /<a href="\/reference">Reference<\/a>/)
  } finally {
    await unlink(fixture)
  }
})
