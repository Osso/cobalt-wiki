import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { createServer } from "node:http"
import { test } from "node:test"
import { fileURLToPath } from "node:url"
import { chromium, expect } from "@playwright/test"
import ts from "typescript"

const sourcePath = fileURLToPath(new URL("../../src/lib/image-check.ts", import.meta.url))
const image = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL/nwAAAABJRU5ErkJggg==",
  "base64"
)
const html = `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8"><title>Image check fixture</title></head>
  <body>
    <label>Image URL <input id="url"></label>
    <button id="check" type="button">Check image</button>
    <p id="error" role="alert"></p>
    <script type="module">
      import { openImageCheck } from "/image-check.js"
      document.querySelector("#check").addEventListener("click", () => {
        document.querySelector("#error").textContent = ""
        try {
          openImageCheck(document.querySelector("#url").value)
        } catch (error) {
          document.querySelector("#error").textContent = error.message
        }
      })
    </script>
  </body>
</html>`

/** @param {import("node:http").ServerResponse} response */
function sendImage(response) {
  response.writeHead(200, { "content-type": "image/png" })
  response.end(image)
}

/** @param {string} javascript */
function makeServer(javascript) {
  return createServer((request, response) => {
    if (request.method !== "GET") {
      response.writeHead(405).end()
      return
    }
    const path = new URL(request.url ?? "/", "http://127.0.0.1").pathname
    if (path === "/tiny.png") {
      sendImage(response)
      return
    }
    if (path === "/image-check.js") {
      response.writeHead(200, { "content-type": "text/javascript" }).end(javascript)
      return
    }
    if (path === "/") {
      response.writeHead(200, { "content-type": "text/html; charset=utf-8" }).end(html)
      return
    }
    response.writeHead(404).end()
  })
}

/** @param {import("node:http").Server} server */
async function listen(server) {
  await new Promise((resolve, reject) => {
    server.once("error", reject)
    server.listen(0, "127.0.0.1", resolve)
  })
  const address = server.address()
  assert.ok(address && typeof address !== "string", "loopback port required")
  return `http://127.0.0.1:${address.port}`
}

/** @param {import("node:http").Server} server */
async function closeServer(server) {
  await new Promise((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()))
  })
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} url
 */
async function clickCheck(page, url) {
  await page.locator("#url").fill(url)
  const popupPromise = page.waitForEvent("popup")
  await page.getByRole("button", { name: "Check image" }).click()
  return popupPromise
}

test("image check helper opens and closes a real isolated browser popup", async (t) => {
  const source = await readFile(sourcePath, "utf8")
  const javascript = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 }
  }).outputText
  const server = makeServer(javascript)
  /** @type {import("@playwright/test").Browser | undefined} */
  let browser
  /** @type {import("@playwright/test").BrowserContext | undefined} */
  let context
  try {
    const origin = await listen(server)
    browser = await chromium.launch({
      executablePath: "/usr/bin/chromium",
      headless: true
    })
    context = await browser.newContext()
    /** @type {string[]} */
    const denied = []
    await context.route("**/*", async (route) => {
      const request = route.request()
      const url = new URL(request.url())
      if (request.method() === "GET" && url.origin === origin) {
        await route.continue()
        return
      }
      denied.push(`${request.method()} ${request.url()}`)
      await route.abort("blockedbyclient")
    })
    const page = await context.newPage()
    await page.goto(origin)

    await t.test(
      "loaded image renders safe content and close link closes popup",
      async () => {
        const url = `${origin}/tiny.png?label=%3Cscript%3Ewindow.injected%3Dtrue%3C%2Fscript%3E`
        const popup = await clickCheck(page, url)
        await expect(popup).toHaveTitle("Checking image...")
        await expect(popup.getByRole("status")).toHaveText("Image loaded.", {
          timeout: 15000
        })
        assert.equal(
          await popup.locator("#check-image").evaluate((element) => {
            if (!(element instanceof HTMLImageElement)) throw new Error("image required")
            return element.naturalWidth
          }),
          1
        )
        await expect(popup.locator("img")).toHaveCount(1)
        await expect(popup.locator("script")).toHaveCount(0)
        assert.equal(await popup.locator("#check-image").getAttribute("src"), url)
        assert.equal(
          await popup.evaluate(() => Reflect.get(window, "injected")),
          undefined
        )
        const closed = popup.waitForEvent("close")
        await popup.getByRole("link", { name: "close this window" }).click()
        await closed
      }
    )

    await t.test("invalid scheme reports error without opening popup", async () => {
      const count = context.pages().length
      await page.locator("#url").fill("javascript:alert(1)")
      await page.getByRole("button", { name: "Check image" }).click()
      await expect(page.getByRole("alert")).toContainText("valid HTTP or HTTPS image URL")
      assert.equal(context.pages().length, count)
    })

    await t.test("missing image reports unavailable", async () => {
      const popup = await clickCheck(page, `${origin}/missing.png`)
      await expect(popup.getByRole("status")).toHaveText("Image unavailable.", {
        timeout: 15000
      })
      assert.equal(
        await popup.locator("#check-image").evaluate((element) => {
          if (!(element instanceof HTMLImageElement)) throw new Error("image required")
          return element.naturalWidth
        }),
        0
      )
      await popup.close()
    })
    assert.deepEqual(denied, [], "browser must make only loopback GET requests")
  } finally {
    try {
      await context?.close()
    } finally {
      try {
        await browser?.close()
      } finally {
        if (server.listening) await closeServer(server)
      }
    }
  }
})
