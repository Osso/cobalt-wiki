import assert from "node:assert/strict"
import { writeFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

// Read-only retained member fixture. Cookies stay in memory; no login or page writes.
const origin = process.env.COBALT_LOCAL_PREVIEW
assert.ok(origin, "COBALT_LOCAL_PREVIEW must name the deployed authorized origin")
const cdpEndpoint = process.env.COBALT_CDP_ENDPOINT ?? "http://127.0.0.1:9222"
const slug = "membersonly:tag-capacity-2000"
const tagCount = 2000
const tagStringLength = 35999
const overflowTolerance = 1

/** @param {import("@playwright/test").Page} page */
async function measureTags(page) {
  return page.locator(".page-tags").evaluate((container) => {
    const links = Array.from(container.querySelectorAll("span > a"))
    const bounds = container.getBoundingClientRect()
    const span = container.querySelector("span")
    const style = getComputedStyle(span)
    return {
      clientWidth: container.clientWidth,
      scrollWidth: container.scrollWidth,
      pageWidth: document.documentElement.scrollWidth,
      containerLeft: bounds.left,
      containerRight: bounds.right,
      innerDisplay: style.display,
      innerFlexWrap: style.flexWrap,
      innerMinWidth: style.minWidth,
      rows: new Set(links.map((link) => Math.round(link.getBoundingClientRect().top)))
        .size,
      fragmentedLinks: links.filter((link) => link.getClientRects().length !== 1).length,
      controls: Array.from(
        document.querySelectorAll(
          "#page-options-container a, #page-options-container button"
        )
      ).map((control) => ({
        text: control.textContent,
        href: control.getAttribute("href")
      })),
      tags: links.map((link) => {
        const rect = link.getBoundingClientRect()
        return {
          text: link.textContent,
          href: link.getAttribute("href"),
          width: rect.width,
          height: rect.height,
          left: rect.left,
          right: rect.right
        }
      })
    }
  })
}

/** @param {Awaited<ReturnType<typeof measureTags>>} measurement */
function assertTagIdentity(measurement) {
  assert.equal(measurement.tags.length, tagCount)
  const texts = measurement.tags.map((tag) => tag.text)
  assert.equal(new Set(texts).size, tagCount, "all fixture tags remain unique")
  assert.equal(texts.join(" ").length, tagStringLength)
  for (const tag of measurement.tags) {
    assert.ok(tag.width > 0 && tag.height > 0, `tag ${tag.text} remains displayed`)
    assert.ok(tag.href, `tag ${tag.text} retains its link`)
    const target = new URL(tag.href, origin)
    assert.equal(target.origin, new URL(origin).origin)
    assert.equal(decodeURIComponent(target.pathname), `/system:page-tags/tag/${tag.text}`)
  }
}

/** @param {Awaited<ReturnType<typeof measureTags>>} measurement */
function assertAtomicWrapping(measurement) {
  assert.equal(
    measurement.fragmentedLinks,
    0,
    `${measurement.fragmentedLinks} tag links split across line boxes; tags must wrap between links`
  )
  assert.ok(measurement.rows > 1, "tags occupy multiple rows")
  assert.ok(
    measurement.scrollWidth <= measurement.clientWidth + overflowTolerance,
    `tag container overflows (${measurement.scrollWidth} > ${measurement.clientWidth})`
  )
  for (const tag of measurement.tags) {
    assert.ok(tag.left >= measurement.containerLeft - overflowTolerance)
    assert.ok(tag.right <= measurement.containerRight + overflowTolerance)
  }
}

/** @param {Awaited<ReturnType<typeof measureTags>>} measurement */
function identities(measurement) {
  return measurement.tags.map(({ text, href }) => ({ text, href }))
}

test("2,000 tag links wrap atomically at desktop and fixed-page mobile widths", async (t) => {
  const browser = await chromium.connectOverCDP(cdpEndpoint)
  let context
  try {
    const authorizedContext = browser.contexts()[0]
    assert.ok(authorizedContext, "existing authorized browser context required")
    const cookies = await authorizedContext.cookies(origin)
    assert.ok(
      cookies.length,
      "existing origin cookies required; no anonymous layout probe"
    )
    context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
    await context.addCookies(cookies)
    const page = await context.newPage()
    const response = await page.goto(`${origin}/${slug}`, { waitUntil: "networkidle" })
    assert.equal(response?.status(), 200, "authorized fixture must return HTTP 200")
    await expect(page.locator(".page-tags > span > a")).toHaveCount(tagCount)
    const original = await measureTags(page)
    assertTagIdentity(original)
    assert.ok(original.controls.length, "surrounding page controls remain present")
    for (const width of [1440, 390]) {
      await t.test(`${width}px viewport`, async () => {
        await page.setViewportSize({ width, height: 900 })
        await page.locator(".page-tags").scrollIntoViewIfNeeded()
        const measurement = await measureTags(page)
        const screenshot = `/tmp/claude/tag-wrap-${width}.png`
        const { tags, controls, ...metrics } = measurement
        console.log(
          JSON.stringify({
            width,
            height: 900,
            url: page.url(),
            tags: tags.length,
            ...metrics,
            screenshot
          })
        )
        const session = await context.newCDPSession(page)
        try {
          const clip = await page.evaluate(() => ({
            x: window.scrollX,
            y: window.scrollY,
            width: window.innerWidth,
            height: window.innerHeight,
            scale: 1 / window.devicePixelRatio
          }))
          const capture = await session.send("Page.captureScreenshot", {
            format: "png",
            captureBeyondViewport: false,
            clip
          })
          await writeFile(screenshot, Buffer.from(capture.data, "base64"))
        } finally {
          await session.detach()
        }
        assertTagIdentity(measurement)
        assert.deepEqual(identities(measurement), identities(original))
        assert.deepEqual(controls, original.controls, "viewport preserves page controls")
        assert.equal(
          measurement.clientWidth,
          original.clientWidth,
          "fixed content width preserved"
        )
        if (width === 390) {
          assert.ok(
            measurement.pageWidth >= 1000,
            "fixed-width mobile page remains pannable"
          )
        }
        assertAtomicWrapping(measurement)
      })
    }
  } finally {
    await context?.close()
    // connectOverCDP close disconnects its transport, not the shared Chromium process.
    // Playwright Browser.close docs; installed Chromium CDP transport closes its socket.
    await browser.close()
  }
})
