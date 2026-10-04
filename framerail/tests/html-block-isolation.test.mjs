import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { createServer } from "node:http"
import { test } from "node:test"
import { chromium } from "@playwright/test"

// Input is captured from the actual WWS router + HTTP Deepwell/S3 fixture.
// No hand-written CSP or iframe sandbox attribute can mask a route regression.
assert.ok(
  process.env.WWS_DOCUMENT_FIXTURES,
  "WWS_DOCUMENT_FIXTURES route captures required"
)
const fixturePaths = process.env.WWS_DOCUMENT_FIXTURES.split(",")

for (const fixturePath of fixturePaths) {
  test(`WWS document ${fixturePath} scripts run without same-origin privileges`, async () => {
    const fixture = JSON.parse(await readFile(fixturePath, "utf8"))
    assert.ok(fixture.path, "captured WWS route path required")
    const documentPath = fixture.path
    const apiRequests = []
    const server = createServer((request, response) => {
      if (request.url === documentPath) {
        response.writeHead(200, fixture.headers)
        response.end(fixture.body)
      } else if (request.url === "/api/probe") {
        apiRequests.push({
          origin: request.headers.origin,
          cookie: request.headers.cookie
        })
        // Authenticated gateway API: no cross-origin read permission.
        response.writeHead(200, { "Content-Type": "text/plain" })
        response.end("private-api-response")
      } else {
        response.writeHead(200, {
          "Content-Type": "text/html",
          "Set-Cookie": "wikijump_token=browser-secret; Path=/; SameSite=Lax"
        })
        response.end(`<!doctype html><div id="secret">parent-secret</div>
<script>window.addEventListener('message', event => {
  window.proof = { ...event.data, origin: event.origin }
})</script><iframe src="${documentPath}"></iframe>`)
      }
    })
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve))
    let browser
    try {
      browser = await chromium.launch({
        executablePath: process.env.CHROMIUM_PATH || "/usr/bin/chromium",
        headless: true
      })
      const page = await browser.newPage()
      await page.goto(`http://127.0.0.1:${server.address().port}/`)
      await page.waitForFunction(() => window.proof !== undefined)
      const proof = await page.evaluate(() => window.proof)
      console.log(JSON.stringify({ proof, apiRequests }))
      assert.equal(proof.scripts, true)
      assert.equal(proof.parent, "SecurityError")
      assert.equal(proof.cookie, "SecurityError")
      assert.equal(proof.api, "TypeError")
      assert.equal(proof.credentialedApi, "TypeError")
      assert.equal(proof.origin, "null")
      assert.equal(apiRequests.length, 2, "sandbox does not prohibit outgoing requests")
      assert.equal(apiRequests[0].origin, "null")
      assert.equal(
        apiRequests[0].cookie,
        undefined,
        "default same-origin credentials exclude opaque origin"
      )
      // Direct navigation also receives document isolation, not an iframe attribute.
      await page.goto(`http://127.0.0.1:${server.address().port}${documentPath}`)
      const directCookie = await page.evaluate(() => {
        try {
          return document.cookie
        } catch (error) {
          return error.name
        }
      })
      assert.equal(directCookie, "SecurityError")
      // credentials:include is not claimed to be blocked; API authorization/CSRF
      // protection remains the API's responsibility, even when CORS denies reads.
    } finally {
      if (browser) await browser.close()
      await new Promise((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve()))
      )
    }
  })
}
