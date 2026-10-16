import assert from "node:assert/strict"
import { readFileSync } from "node:fs"
import { createServer, request } from "node:http"
import { test } from "node:test"
import { chromium } from "@playwright/test"

const backend = process.env.COBALT_E2E_RPC ?? "http://127.0.0.1:2749/jsonrpc"
const frontend = process.env.COBALT_E2E_FRONTEND ?? "http://127.0.0.1:3394"
const sessionFile = process.env.COBALT_E2E_SESSION_FILE
assert.ok(sessionFile, "Set COBALT_E2E_SESSION_FILE to an owned local test session")
assert.equal(new URL(backend).hostname, "127.0.0.1", "Never run against production")
assert.equal(new URL(frontend).hostname, "127.0.0.1", "Never run against production")
const token = readFileSync(sessionFile, "utf8").trim()
const site = 6000000

/** @param {string} method @param {object} params */
async function rpc(method, params) {
  const response = await fetch(backend, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "X-Deepwell-Session-Token": token,
      "X-Deepwell-Site-Id": String(site)
    },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params })
  })
  const result = await response.json()
  assert.ok(!result.error, JSON.stringify(result.error))
  return result.result
}

function createGateway() {
  return createServer((incoming, outgoing) => {
    const proxy = request(
      new URL(incoming.url ?? "/", frontend),
      {
        method: incoming.method,
        headers: {
          ...incoming.headers,
          "X-Wikijump-Site-Id": String(site),
          "X-Wikijump-Site-Slug": "cobalt-company",
          "X-Wikijump-Target-Server": "main",
          "X-Wikijump-User-Id": "",
          authorization: ""
        }
      },
      (response) => {
        outgoing.writeHead(response.statusCode ?? 502, response.headers)
        response.pipe(outgoing)
      }
    )
    proxy.on("error", (error) => {
      outgoing.writeHead(502)
      outgoing.end(error.message)
    })
    incoming.pipe(proxy)
  })
}

test(
  "real browser confirms destination tags through action, RPC and worker",
  { timeout: 120_000 },
  async (t) => {
    const prefix = `zz-loop-${Date.now()}`
    const oldName = `${prefix}-old`
    const newName = `${prefix}-new`
    const actor = await rpc("session_get", [token])
    assert.ok(actor && !actor.restricted)
    /** @type {number[]} */
    const fixtures = []
    for (const [slug, tags] of [
      [`character:${oldName}`, [oldName]],
      [`writing:${prefix}-overlap`, [oldName, `re:${oldName}`, newName, `re:${newName}`]],
      [`writing:${prefix}-existing`, [`re:${newName}`]]
    ]) {
      const page = await rpc("page_import", {
        site_id: site,
        user_id: actor.user_id,
        slug,
        title: slug,
        wikitext: slug.startsWith("character:")
          ? "name: Owned local confirmation fixture\n"
          : `characters: '${slug.endsWith("-existing") ? "" : oldName}'\ncontent: Owned local confirmation fixture\n`,
        alt_title: null,
        layout: "wikidot",
        tags,
        bypass_filter: true,
        revision_comments: "local browser confirmation regression",
        ip_address: "127.0.0.1"
      })
      fixtures.push(page.page_id)
    }
    const gateway = createGateway()
    await new Promise((resolve) =>
      gateway.listen(3089, "127.0.0.1", () => resolve(undefined))
    )
    const browser = await chromium.launch({
      executablePath: process.env.CHROMIUM_PATH ?? "/usr/bin/chromium",
      headless: true
    })
    try {
      const context = await browser.newContext()
      await context.addCookies([
        { name: "wikijump_token", value: token, url: "http://127.0.0.1:3089" }
      ])
      const page = await context.newPage()
      await page.addInitScript(() => {
        // Reproduce the payload emitted by tabs loaded before acknowledgment was added.
        const originalFetch = window.fetch.bind(window)
        window.fetch = (url, init) => {
          if (
            String(url).includes("?/characterRenameStart") &&
            init?.body instanceof FormData
          ) {
            if (sessionStorage.getItem("simulate-predeploy-tab") === "yes") {
              init.body.delete("allowExistingTags")
            }
          }
          return originalFetch(url, init)
        }
      })
      const initial = await page.goto(`http://127.0.0.1:3089/character:${oldName}`)
      assert.equal(initial?.status(), 200, await page.locator("body").innerText())
      await page.locator("#more-options-button").click()
      await page.locator("#character-rename-button").click()
      const preview = page.locator('section[aria-label="Rename preview"]')
      const name = page.locator('input[name="newName"]')
      await t.test(
        "a predeploy tab is told to reload instead of previewing in a loop",
        async () => {
          await page.evaluate(() =>
            sessionStorage.setItem("simulate-predeploy-tab", "yes")
          )
          const messages = []
          for (const attempt of [0, 1]) {
            await name.fill(newName)
            await page.getByRole("button", { name: "Preview", exact: true }).click()
            await preview.waitFor()
            assert.match(await preview.innerText(), /2\s+pages already use/)
            await preview
              .getByRole("button", {
                name: "Confirm rename and keep existing tags",
                exact: true
              })
              .click()
            const error = page.getByRole("alert")
            await error.waitFor()
            messages.push(await error.innerText())
            assert.equal(
              (await rpc("page_get", { site_id: site, page: fixtures[0] })).slug,
              `character:${oldName}`,
              `attempt ${attempt}`
            )
          }
          for (const message of messages) assert.match(message, /Reload this page/)
        }
      )
      await t.test(
        "fresh preview confirmation completes the real background rename",
        async () => {
          await page.evaluate(() => sessionStorage.removeItem("simulate-predeploy-tab"))
          await page.reload()
          await page.locator("#more-options-button").click()
          await page.locator("#character-rename-button").click()
          await name.fill(newName)
          await page.getByRole("button", { name: "Preview", exact: true }).click()
          await preview.waitFor()
          const before = await rpc("page_get", { site_id: site, page: fixtures[2] })
          await preview
            .getByRole("button", {
              name: "Confirm rename and keep existing tags",
              exact: true
            })
            .click()
          await page.waitForURL(`**/character:${newName}`, { timeout: 60_000 })
          const renamed = await rpc("page_get", { site_id: site, page: fixtures[0] })
          const overlap = await rpc("page_get", { site_id: site, page: fixtures[1] })
          const existing = await rpc("page_get", { site_id: site, page: fixtures[2] })
          assert.equal(renamed.slug, `character:${newName}`)
          assert.deepEqual(renamed.tags, [newName])
          assert.deepEqual([...overlap.tags].sort(), [newName, `re:${newName}`].sort())
          assert.deepEqual(existing.tags, [`re:${newName}`])
          assert.equal(existing.revision_id, before.revision_id)
          await page.screenshot({ path: "/tmp/cobalt-rename-local-confirmed.jpg" })
        }
      )
    } finally {
      await browser.close()
      gateway.closeAllConnections()
      await new Promise((resolve) => gateway.close(resolve))
      for (const id of fixtures) {
        const page = await rpc("page_get", { site_id: site, page: id })
        await rpc("page_delete", {
          site_id: site,
          page: id,
          user_id: actor.user_id,
          last_revision_id: page.revision_id,
          revision_comments: "owned local fixture cleanup",
          ip_address: "127.0.0.1"
        })
      }
    }
  }
)
