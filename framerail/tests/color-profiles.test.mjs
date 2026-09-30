import assert from "node:assert/strict"
import { readFile, writeFile, unlink } from "node:fs/promises"
import { test } from "node:test"
import { chromium } from "@playwright/test"
import { compile, preprocess } from "svelte/compiler"
import { sveltePreprocess } from "svelte-preprocess"
import { createRawSnippet } from "svelte"
import { render } from "svelte/server"

const navigation =
  '<ul><li><strong><a href="/roster">Roster</a></strong><ul><li><a href="/roster-by-player">By player</a></li></ul></li></ul>'

test("account color profiles change tab colors without changing geometry", async () => {
  const source = await readFile(
    new URL("../src/lib/sigma-esque/wikidot.svelte", import.meta.url),
    "utf8"
  )
  const prepared = await preprocess(source, sveltePreprocess(), {
    filename: "wikidot.svelte"
  })
  const compiled = compile(prepared.code, {
    generate: "server",
    filename: "wikidot.svelte"
  })
  const fixture = new URL(`./.color-profiles-${process.pid}.mjs`, import.meta.url)
  await writeFile(fixture, compiled.js.code)
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    headless: true
  })
  try {
    const { default: Component } = await import(fixture.href)
    const page = await browser.newPage()
    const geometry = []
    for (const [colorProfile, colors] of [
      [
        "classic",
        { fill: "rgb(168, 215, 234)", hover: "rgb(214, 243, 255)", text: "rgb(0, 0, 0)" }
      ],
      [
        "current",
        {
          fill: "rgb(188, 201, 242)",
          hover: "rgb(224, 230, 251)",
          text: "rgb(11, 26, 56)"
        }
      ],
      [
        undefined,
        {
          fill: "rgb(188, 201, 242)",
          hover: "rgb(224, 230, 251)",
          text: "rgb(11, 26, 56)"
        }
      ]
    ]) {
      const { body } = render(Component, {
        props: {
          colorProfile,
          topBar: createRawSnippet(() => ({ render: () => navigation }))
        }
      })
      await page.setContent(`<style>${compiled.css.code}</style>${body}`)
      await page.mouse.move(800, 600)
      const tab = page.locator("#top-bar > ul > li")
      const label = tab.locator(":scope > strong > a")
      const menu = tab.locator(":scope > ul")
      const menuLink = menu.locator("a")
      assert.equal(
        await tab.evaluate((element) => getComputedStyle(element).backgroundColor),
        colors.fill
      )
      assert.equal(
        await label.evaluate((element) => getComputedStyle(element).color),
        colors.text
      )
      assert.equal(
        await menu.evaluate((element) => getComputedStyle(element).backgroundColor),
        colors.fill
      )
      assert.equal(
        await menuLink.evaluate((element) => getComputedStyle(element).color),
        colors.text
      )
      geometry.push(
        await tab.evaluate((element) => {
          const style = getComputedStyle(element)
          const box = element.getBoundingClientRect()
          return {
            width: box.width,
            height: box.height,
            padding: style.padding,
            radius: style.borderRadius
          }
        })
      )
      await label.hover()
      assert.equal(
        await tab.evaluate((element) => getComputedStyle(element).backgroundColor),
        colors.hover
      )
      await menuLink.hover()
      assert.equal(
        await menuLink.evaluate((element) => getComputedStyle(element).backgroundColor),
        colors.hover
      )
    }
    assert.deepEqual(geometry[0], geometry[1])
    assert.deepEqual(geometry[1], geometry[2])
  } finally {
    await browser.close()
    await unlink(fixture)
  }
})
