import assert from "node:assert/strict"
import { readFile, writeFile, unlink } from "node:fs/promises"
import { test } from "node:test"
import { chromium } from "@playwright/test"
import { compile, preprocess } from "svelte/compiler"
import { sveltePreprocess } from "svelte-preprocess"
import { render } from "svelte/server"

const texture = `data:image/svg+xml,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><path fill="#123456" d="M0 0h20v20H0z"/><path fill="#245678" d="M10 0h10v20H10z"/></svg>')}`

/** @param {import("@playwright/test").Page} page */
async function sampleScreenshotColors(page) {
  const screenshot = await page.screenshot()
  return page.evaluate(async (base64) => {
    const image = new Image()
    image.src = `data:image/png;base64,${base64}`
    await image.decode()
    const canvas = document.createElement("canvas")
    canvas.width = image.width
    canvas.height = image.height
    const context = canvas.getContext("2d")
    if (!context) {
      throw new Error("Screenshot canvas must provide a 2D context")
    }
    const viewport = visualViewport
    if (!viewport) {
      throw new Error("Screenshot sampling requires a visual viewport")
    }
    context.drawImage(image, 0, 0)
    const scale = viewport.scale
    const points =
      image.width === 1440 ? [105, 115, 505, 905, 1405] : [105, 115, 505, 905]
    return points.map((x) => {
      const pixel = context.getImageData(
        Math.round((x - viewport.pageLeft) * scale),
        Math.round(50 * scale),
        1,
        1
      ).data
      return [...pixel].slice(0, 3)
    })
  }, screenshot.toString("base64"))
}

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
const css = compiled.css
assert.ok(css, "layout CSS must compile")
const fixture = new URL(`./.mobile-background-${process.pid}.mjs`, import.meta.url)
await writeFile(fixture, compiled.js.code)
const { default: Component } = await import(fixture.href)

try {
  for (const width of [390, 1440]) {
    test(`Cobalt texture paints across the 1000px header at viewport ${width}`, async () => {
      const browser = await chromium.launch({
        executablePath: "/usr/bin/chromium",
        headless: true
      })
      try {
        const page = await browser.newPage({
          viewport: { width, height: 844 },
          isMobile: width === 390,
          hasTouch: width === 390,
          deviceScaleFactor: 1
        })
        const { body } = render(Component, { props: { siteSlug: "cobalt-company" } })
        await page.setContent(`<!doctype html><meta name="viewport" content="width=device-width, initial-scale=1"><style>
          html { background: white; }
          body { margin: 0; background-image: url("${texture}"); background-position: top left; background-repeat: repeat; background-attachment: fixed; }
          #container { min-width: 1000px; max-width: 1000px; margin: 0 auto; height: 1800px; }
          #header { height: 100px; }
          ${css.code}
        </style><div>${body}</div>`)
        if (width === 390) {
          const session = await page.context().newCDPSession(page)
          await session.send("Emulation.setPageScaleFactor", {
            pageScaleFactor: 390 / 1000
          })
        }
        const geometry = await page.evaluate(() => {
          const container = document.querySelector("#container")
          if (!container) {
            throw new Error("Background fixture must contain #container")
          }
          const viewport = visualViewport
          if (!viewport) {
            throw new Error("Background geometry requires a visual viewport")
          }
          return {
            bodyWidth: document.body.getBoundingClientRect().width,
            containerWidth: container.getBoundingClientRect().width,
            viewportWidth: innerWidth,
            scale: viewport.scale,
            attachment: getComputedStyle(document.body).backgroundAttachment,
            size: getComputedStyle(document.body).backgroundSize
          }
        })
        console.log(JSON.stringify({ width, ...geometry }))
        assert.equal(geometry.containerWidth, 1000, "retain desktop-width layout")
        assert.equal(geometry.size, "auto", "retain intrinsic texture size")
        const colors = await sampleScreenshotColors(page)
        console.log(JSON.stringify({ width, colors }))
        assert.deepEqual(
          colors,
          [
            [18, 52, 86],
            [36, 86, 120],
            [18, 52, 86],
            [18, 52, 86],
            ...(width === 1440 ? [[18, 52, 86]] : [])
          ],
          "intrinsic texture must repeat across the header"
        )
        assert.equal(
          geometry.bodyWidth,
          Math.max(width, 1000),
          "body must cover the full layout"
        )
        assert.equal(geometry.attachment, "fixed", "retain fixed background attachment")
        if (width === 390) {
          const session = await page.context().newCDPSession(page)
          await session.send("Emulation.setPageScaleFactor", { pageScaleFactor: 1 })
          await session.send("Input.synthesizeScrollGesture", {
            x: 200,
            y: 200,
            xDistance: -600,
            yDistance: -300,
            gestureSourceType: "mouse",
            speed: 800
          })
        } else {
          await page.evaluate(() => scrollTo(0, 300))
        }
        const scroll = await page.evaluate(() => {
          const viewport = visualViewport
          if (!viewport) {
            throw new Error("Scroll measurements require a visual viewport")
          }
          return { left: viewport.pageLeft, top: viewport.pageTop }
        })
        console.log(JSON.stringify({ width, scroll }))
        assert.ok(scroll.top >= 200, "fixture must actually scroll vertically")
        if (width === 390) {
          assert.ok(
            scroll.left >= 500,
            "fixture must pan to the wide layout's right side"
          )
        }
        const scrolled = await page.screenshot()
        // Sample the visible right side after scrolling, away from all chrome.
        const rightPixel = await page.evaluate(async (base64) => {
          const image = new Image()
          image.src = `data:image/png;base64,${base64}`
          await image.decode()
          const canvas = document.createElement("canvas")
          canvas.width = image.width
          canvas.height = image.height
          const context = canvas.getContext("2d")
          if (!context) {
            throw new Error("Screenshot canvas must provide a 2D context")
          }
          context.drawImage(image, 0, 0)
          return [...context.getImageData(image.width - 20, 20, 1, 1).data].slice(0, 3)
        }, scrolled.toString("base64"))
        assert.ok(
          rightPixel[2] > rightPixel[1] && rightPixel[1] > rightPixel[0],
          `scrolled right edge must remain textured: ${rightPixel}`
        )
      } finally {
        await browser.close()
      }
    })
  }
  for (const siteSlug of ["scp-wiki", undefined]) {
    test(`other Wikidot site ${siteSlug ?? "unspecified"} retains its body width`, async () => {
      const browser = await chromium.launch({
        executablePath: "/usr/bin/chromium",
        headless: true
      })
      try {
        const page = await browser.newPage({
          viewport: { width: 390, height: 844 },
          isMobile: true
        })
        const { body } = render(Component, { props: { siteSlug } })
        await page.setContent(`<!doctype html><meta name="viewport" content="width=device-width, initial-scale=1"><style>
          body { margin: 0; }
          #container { width: 640px; }
          ${css.code}
        </style><div>${body}</div>`)
        assert.equal(
          await page.evaluate(() => document.body.getBoundingClientRect().width),
          390
        )
        assert.equal(
          await page
            .locator("#container")
            .evaluate((element) => element.getBoundingClientRect().width),
          640
        )
      } finally {
        await browser.close()
      }
    })
  }
} finally {
  await unlink(fixture)
}
