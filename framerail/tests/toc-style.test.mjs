import assert from "node:assert/strict"
import { readFile, writeFile, unlink } from "node:fs/promises"
import { test } from "node:test"
import { chromium } from "@playwright/test"
import { compile, preprocess } from "svelte/compiler"
import { sveltePreprocess } from "svelte-preprocess"
import { createRawSnippet } from "svelte"
import { render } from "svelte/server"

const names = [
  "Alli",
  "Aly",
  "Athena",
  "Blade",
  "Chem",
  "Disknight",
  "E",
  "inkie",
  "Jessa",
  "Josh",
  "Luridel",
  "Mei",
  "Mishell",
  "MrMook",
  "OzmaAsimov",
  "Pelican",
  "Saaron",
  "Teensy",
  "Vond"
]

test("generated roster TOC inherits Wikidot panel styling and keeps floating links usable", async () => {
  const source = await readFile(
    new URL("../src/lib/sigma-esque/wikidot.svelte", import.meta.url),
    "utf8"
  )
  const toc = await readFile(
    new URL("./fixtures/roster-toc.html", import.meta.url),
    "utf8"
  )
  const prepared = await preprocess(source, sveltePreprocess(), {
    filename: "wikidot.svelte"
  })
  const compiled = compile(prepared.code, {
    generate: "server",
    filename: "wikidot.svelte"
  })
  assert.ok(compiled.css, "layout CSS must compile")
  const css = compiled.css.code
  const fixture = new URL(`./.toc-style-${process.pid}.mjs`, import.meta.url)
  await writeFile(fixture, compiled.js.code)
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const { default: Component } = await import(fixture.href)
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
    const { body } = render(Component, {
      props: {
        content: createRawSnippet(() => ({
          render: () => `<div id="page-content">${toc}</div>`
        }))
      }
    })
    await page.setContent(
      `<style>body { background: #101833; } #page-content { padding-top: 200px; } ${css}</style>${body}`
    )
    const panel = page.locator("#wj-toc")
    assert.deepEqual(
      (await panel.locator("#wj-toc-list a").allTextContents()).map((name) =>
        name.trim()
      ),
      names
    )
    const appearance = await panel.evaluate((element) => {
      const panelStyle = getComputedStyle(element)
      const title = element.querySelector(".title")
      const list = element.querySelector("#wj-toc-list")
      const action = element.querySelector("#wj-toc-action-bar")
      if (!title || !list || !action) throw new Error("generated TOC structure missing")
      const titleStyle = getComputedStyle(title)
      const listStyle = getComputedStyle(list)
      const actionStyle = getComputedStyle(action)
      return {
        display: panelStyle.display,
        background: panelStyle.backgroundColor,
        borderColor: panelStyle.borderTopColor,
        borderStyle: panelStyle.borderTopStyle,
        borderWidth: panelStyle.borderTopWidth,
        padding: panelStyle.padding,
        marginTop: panelStyle.marginTop,
        titleWeight: titleStyle.fontWeight,
        titleAlignment: titleStyle.textAlign,
        titlePadding: titleStyle.padding,
        titleMargin: titleStyle.margin,
        listMargin: listStyle.margin,
        actionPadding: actionStyle.padding
      }
    })
    assert.deepEqual(appearance, {
      display: "table",
      background: "rgb(247, 247, 247)",
      borderColor: "rgb(128, 128, 128)",
      borderStyle: "dashed",
      borderWidth: "1px",
      padding: "0px 6.4px",
      marginTop: "12.8px",
      titleWeight: "700",
      titleAlignment: "center",
      titlePadding: "0px 25.6px",
      titleMargin: "2.56px 0px",
      listMargin: "6.4px 0px",
      actionPadding: "2px"
    })
    const before = await panel.boundingBox()
    assert.ok(before, "TOC must have a visible box")
    await page.evaluate(() => scrollTo(0, 600))
    assert.deepEqual(
      await panel.boundingBox(),
      before,
      "existing fixed wrapper remains fixed while scrolling"
    )
    await panel.getByRole("link", { name: "Luridel", exact: true }).click()
    assert.equal(await page.evaluate(() => location.hash), "#toc10")
    assert.equal(await page.locator("#toc10").textContent(), "Luridel")
    assert.equal(await panel.locator("#wj-toc-list a").count(), 19)
  } finally {
    await browser.close()
    await unlink(fixture)
  }
})
