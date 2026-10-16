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

/** @param {ReturnType<typeof createGateway>} gateway */
async function startGateway(gateway) {
  await new Promise((resolve, reject) => {
    gateway.once("error", reject)
    gateway.listen(3089, "127.0.0.1", () => resolve(undefined))
  })
}

/** @param {ReturnType<typeof createGateway>} gateway */
async function closeGateway(gateway) {
  if (!gateway.listening) return
  gateway.closeAllConnections()
  await new Promise((resolve, reject) => {
    gateway.close((error) => (error ? reject(error) : resolve(undefined)))
  })
}

/** @param {number[]} fixtures @param {number} userId @param {string} prefix */
async function importFixtures(fixtures, userId, prefix) {
  const oldName = `${prefix}-old`
  const newName = `${prefix}-new`
  const definitions = [
    {
      slug: `character:${oldName}`,
      tags: [oldName],
      wikitext: "name: Owned local confirmation fixture\n"
    },
    {
      slug: `writing:${prefix}-overlap`,
      tags: [oldName, `re:${oldName}`, newName, `re:${newName}`],
      wikitext: `characters: '${oldName}'\ncontent: Owned local confirmation fixture\n`
    },
    {
      slug: `writing:${prefix}-existing`,
      tags: [`re:${newName}`],
      wikitext: "characters: ''\ncontent: Existing destination tag only\n"
    }
  ]
  for (const definition of definitions) {
    const page = await rpc("page_import", {
      site_id: site,
      user_id: userId,
      ...definition,
      title: definition.slug,
      alt_title: null,
      layout: "wikidot",
      bypass_filter: true,
      revision_comments: "local browser confirmation regression",
      ip_address: "127.0.0.1"
    })
    fixtures.push(page.page_id)
  }
}

/** @param {number[]} fixtures @param {number} userId */
async function deleteFixtures(fixtures, userId) {
  await Promise.all(
    fixtures.map(async (id) => {
      const page = await rpc("page_get", { site_id: site, page: id })
      await rpc("page_delete", {
        site_id: site,
        page: id,
        user_id: userId,
        last_revision_id: page.revision_id,
        revision_comments: "owned local fixture cleanup",
        ip_address: "127.0.0.1"
      })
    })
  )
}

/** @param {import("@playwright/test").Browser} browser */
async function createAuthenticatedPage(browser) {
  const context = await browser.newContext()
  await context.addCookies([
    { name: "wikijump_token", value: token, url: "http://127.0.0.1:3089" }
  ])
  const page = await context.newPage()
  await page.addInitScript(() => {
    // Simulate the original client's missing field, not an old frontend bundle.
    const originalFetch = window.fetch.bind(window)
    window.fetch = (url, init) => {
      if (
        String(url).includes("?/characterRenameStart") &&
        init?.body instanceof FormData &&
        sessionStorage.getItem("simulate-predeploy-tab") === "yes"
      ) {
        init.body.delete("allowExistingTags")
      }
      return originalFetch(url, init)
    }
  })
  return page
}

/** @param {import("@playwright/test").Page} page */
async function openRenamePane(page) {
  await page.locator("#more-options-button").click()
  await page.locator("#character-rename-button").click()
}

/** @param {import("@playwright/test").Page} page @param {string} newName */
async function previewRename(page, newName) {
  await page.locator('input[name="newName"]').fill(newName)
  await page.getByRole("button", { name: "Preview", exact: true }).click()
  const preview = page.locator('section[aria-label="Rename preview"]')
  await preview.waitFor()
  assert.match(await preview.innerText(), /2\s+pages already use/)
  return preview
}

/** @param {import("@playwright/test").Locator} preview */
async function confirmRename(preview) {
  await preview
    .getByRole("button", { name: "Confirm rename and keep existing tags", exact: true })
    .click()
}

/**
 * @param {import("@playwright/test").Page} page @param {string} prefix
 *   @param {number} character
 */
async function assertOldPayloadRecovery(page, prefix, character) {
  await page.evaluate(() => sessionStorage.setItem("simulate-predeploy-tab", "yes"))
  const messages = []
  for (const attempt of [0, 1]) {
    await confirmRename(await previewRename(page, `${prefix}-new`))
    const error = page.getByRole("alert")
    await error.waitFor()
    messages.push(await error.innerText())
    assert.equal(
      (await rpc("page_get", { site_id: site, page: character })).slug,
      `character:${prefix}-old`,
      `attempt ${attempt}`
    )
  }
  for (const message of messages) assert.match(message, /Reload this page/)
}

/**
 * @param {import("@playwright/test").Page} page @param {string} newName
 *   @param {number[]} fixtures
 */
async function assertConfirmedRename(page, newName, fixtures) {
  await page.evaluate(() => sessionStorage.removeItem("simulate-predeploy-tab"))
  await page.reload()
  await openRenamePane(page)
  const preview = await previewRename(page, newName)
  const before = await rpc("page_get", { site_id: site, page: fixtures[2] })
  await confirmRename(preview)
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

test(
  "real browser confirms destination tags through action, RPC and worker",
  { timeout: 120_000 },
  async (t) => {
    const prefix = `zz-loop-${Date.now()}`
    const actor = await rpc("session_get", [token])
    assert.ok(actor && !actor.restricted)
    /** @type {number[]} */
    const fixtures = []
    const gateway = createGateway()
    /** @type {import("@playwright/test").Browser | undefined} */
    let browser
    try {
      await importFixtures(fixtures, actor.user_id, prefix)
      await startGateway(gateway)
      browser = await chromium.launch({
        executablePath: process.env.CHROMIUM_PATH ?? "/usr/bin/chromium",
        headless: true
      })
      const page = await createAuthenticatedPage(browser)
      const initial = await page.goto(`http://127.0.0.1:3089/character:${prefix}-old`)
      assert.equal(initial?.status(), 200, await page.locator("body").innerText())
      await openRenamePane(page)
      await t.test(
        "a predeploy tab is told to reload instead of previewing in a loop",
        () => assertOldPayloadRecovery(page, prefix, fixtures[0])
      )
      await t.test(
        "fresh preview confirmation completes the real background rename",
        () => assertConfirmedRename(page, `${prefix}-new`, fixtures)
      )
    } finally {
      await Promise.all([
        browser?.close(),
        closeGateway(gateway),
        deleteFixtures(fixtures, actor.user_id)
      ])
    }
  }
)
