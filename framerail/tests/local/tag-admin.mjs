import assert from "node:assert/strict"
import { randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

// Fixed loopback endpoints only. Every mutation uses newly-created fixture pages
// and random tags; never choose an existing site tag for this destructive test.
const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000
const nonce = randomBytes(8).toString("hex")
const source = `tag-proof-source-${nonce}`
const target = `tag-proof-target-${nonce}`
const kept = `tag-proof-kept-${nonce}`

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} method @param {object} params
 */
async function rpc(request, token, method, params) {
  const response = await request.post(backend, {
    headers: { "X-Deepwell-Site-Id": String(siteId), "X-Deepwell-Session-Token": token },
    data: { jsonrpc: "2.0", id: 1, method, params }
  })
  assert.equal(response.status(), 200)
  const payload = await response.json()
  assert.ok(!payload.error, `${method} failed: ${payload.error?.code}`)
  return payload.result
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug
 */
async function readPage(request, token, slug) {
  const result = await rpc(request, token, "page_view", {
    site_id: siteId,
    session_token: token,
    locales: ["en"],
    route: { slug, extra: "" }
  })
  assert.equal(result.type, "found")
  return result.data
}

/** @param {import("@playwright/test").Page} page @param {string} operation */
async function previewChange(page, operation) {
  await page.locator("#tag-operation").selectOption(operation)
  await page.locator("#source-tag").fill(source)
  if (operation === "rename") await page.locator("#target-tag").fill(target)
  await page.getByRole("button", { name: "Preview affected pages" }).click()
  await expect(
    page.getByRole("heading", { name: "Preview: 2 pages", exact: true })
  ).toBeVisible()
}

test(
  "admin previews, cancels, merges, deletes and undoes only isolated fixture tags",
  { timeout: 180000 },
  async () => {
    const passwordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
    assert.ok(passwordPath, "local admin password file required")
    const browser = await chromium.launch({
      executablePath: "/usr/bin/chromium",
      headless: true
    })
    const context = await browser.newContext()
    const fixtures = []
    let token = ""
    let actorId = 0
    try {
      const page = await context.newPage()
      await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
      await page.locator('#login [name="nameOrEmail"]').fill("cobalt-import")
      await page
        .locator('#login [name="password"]')
        .fill((await readFile(passwordPath, "utf8")).trim())
      await page.locator('#login button[type="submit"]').click()
      await expect(page.locator("#login")).toHaveCount(0)
      token = decodeURIComponent(
        (await context.cookies()).find((cookie) => cookie.name === "wikijump_token")
          ?.value ?? ""
      )
      assert.ok(token)
      const session = await rpc(context.request, token, "session_get", [token])
      actorId = session.user_id
      assert.ok(Number.isSafeInteger(actorId))

      for (let index = 0; index < 2; index++) {
        const slug = `local-tag-admin-proof:page-${index}-${nonce}`
        const tags = index ? [source, target, kept] : [source, kept]
        const created = await rpc(context.request, token, "page_import", {
          site_id: siteId,
          slug,
          title: `Tag admin fixture ${index}`,
          wikitext: "Preserve this fixture body.",
          tags,
          user_id: actorId,
          ip_address: "127.0.0.1",
          revision_comments: "Isolated tag admin fixture",
          layout: "wikidot"
        })
        fixtures.push({
          slug,
          pageId: created.page_id,
          tags: tags.sort(),
          before: await readPage(context.request, token, slug)
        })
      }
      const expectedIds = fixtures.map((f) => f.pageId).sort((a, b) => a - b)
      const snapshot = await rpc(context.request, token, "site_tag_preview", {
        source_tag: source,
        target_tag: target
      })
      assert.deepEqual(
        snapshot.pages.map((p) => p.page_id).sort((a, b) => a - b),
        expectedIds
      )

      await page.goto(`${origin}/-/admin/tags`, { waitUntil: "networkidle" })
      await expect(
        page.getByRole("heading", { name: "Manage tags", exact: true })
      ).toBeVisible()
      await previewChange(page, "rename")
      await page.getByRole("button", { name: "Cancel", exact: true }).click()
      await expect(
        page.getByRole("button", { name: "Confirm rename", exact: true })
      ).toHaveCount(0)
      for (const fixture of fixtures) {
        const stored = await readPage(context.request, token, fixture.slug)
        assert.equal(
          stored.page.latest_revision_id,
          fixture.before.page.latest_revision_id
        )
      }
      await previewChange(page, "rename")
      await page.getByRole("button", { name: "Confirm rename", exact: true }).click()
      await expect(
        page.getByRole("status").filter({ hasText: "Tags changed on 2 pages" })
      ).toBeVisible()
      for (const fixture of fixtures) {
        const stored = await readPage(context.request, token, fixture.slug)
        assert.deepEqual([...stored.page_revision.tags].sort(), [target, kept].sort())
        assert.equal(
          stored.page_revision.wikitext_hash,
          fixture.before.page_revision.wikitext_hash
        )
      }
      const history = await rpc(context.request, token, "site_tag_history", {})
      const operation = history.find(
        (item) => item.source_tag === source && !item.reversed
      )
      assert.ok(operation)
      const article = page
        .locator("article")
        .filter({
          has: page.getByRole("heading", {
            name: `#${operation.operation_id}: ${source} → ${target}`,
            exact: true
          })
        })
      await article.getByText("Undo this operation", { exact: true }).click()
      await article
        .getByRole("button", {
          name: `Confirm undo #${operation.operation_id}`,
          exact: true
        })
        .click()
      await expect(
        page.getByRole("status").filter({ hasText: "undone on 2 pages" })
      ).toBeVisible()
      for (const fixture of fixtures) {
        const stored = await readPage(context.request, token, fixture.slug)
        assert.deepEqual([...stored.page_revision.tags].sort(), fixture.tags)
      }

      await previewChange(page, "delete")
      await page.getByRole("button", { name: "Confirm delete tag", exact: true }).click()
      await expect(
        page.getByRole("status").filter({ hasText: "Tags changed on 2 pages" })
      ).toBeVisible()
      for (const fixture of fixtures) {
        const stored = await readPage(context.request, token, fixture.slug)
        assert.deepEqual(
          [...stored.page_revision.tags].sort(),
          fixture.tags.filter((tag) => tag !== source)
        )
        assert.equal(
          stored.page_revision.wikitext_hash,
          fixture.before.page_revision.wikitext_hash
        )
      }
      const deleted = (await rpc(context.request, token, "site_tag_history", {})).find(
        (item) => item.source_tag === source && !item.reversed
      )
      assert.ok(deleted)
      await rpc(context.request, token, "site_tag_undo", {
        operation_id: deleted.operation_id,
        ip_address: "127.0.0.1"
      })
      for (const fixture of fixtures) {
        const stored = await readPage(context.request, token, fixture.slug)
        assert.deepEqual([...stored.page_revision.tags].sort(), fixture.tags)
      }
      const anonymous = await browser.newContext()
      try {
        const visitor = await anonymous.newPage()
        await visitor.goto(`${origin}/-/admin/tags`, { waitUntil: "networkidle" })
        await expect(
          visitor.getByText("as an administrator of this site.", { exact: false })
        ).toBeVisible()
        await expect(visitor.locator('form[action="?/preview"]')).toHaveCount(0)
      } finally {
        await anonymous.close()
      }
    } finally {
      try {
        for (const fixture of fixtures) {
          assert.ok(
            fixture.slug.startsWith("local-tag-admin-proof:") &&
              fixture.slug.endsWith(nonce)
          )
          const stored = await readPage(context.request, token, fixture.slug)
          assert.equal(stored.page.page_id, fixture.pageId)
          await rpc(context.request, token, "page_delete", {
            site_id: siteId,
            page: fixture.pageId,
            last_revision_id: stored.page.latest_revision_id,
            revision_comments: "Remove isolated tag admin fixture",
            user_id: actorId,
            ip_address: "127.0.0.1"
          })
        }
      } finally {
        await browser.close()
      }
    }
  }
)
