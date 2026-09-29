import assert from "node:assert/strict"
import { randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

// Fixed loopback endpoints only. Mutations use newly-created fixture pages and
// random tags; never choose an existing site tag for this destructive test.
const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000
const nonce = randomBytes(8).toString("hex")
const source = `tag-proof-source-${nonce}`
const target = `tag-proof-target-${nonce}`
const kept = `tag-proof-kept-${nonce}`

/** @typedef {import("@playwright/test").Page} BrowserPage */
/** @typedef {import("@playwright/test").BrowserContext} BrowserContext */
/** @typedef {import("@playwright/test").APIRequestContext} RequestContext */
/** @typedef {import("../../src/lib/server/deepwell/tag-admin").TagOperation} TagOperation */
/** @typedef {import("../../src/lib/server/deepwell/tag-admin").TagPreview} TagPreview */
/**
 * @typedef {Extract<
 *   import("../../src/lib/server/deepwell/views").PageView,
 *   { type: "found" }
 * >["data"]} StoredPage
 */
/**
 * @typedef {{
 *   slug: string
 *   pageId: number
 *   tags: string[]
 *   before: StoredPage
 * }} Fixture
 */
/** @typedef {{ request: RequestContext; token: string; actorId: number }} Actor */

/**
 * @param {RequestContext} request @param {string} token @param {string}
 *   method @param {object} params
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

/** @param {Actor} actor @param {string} slug @returns {Promise<StoredPage>} */
async function readPage(actor, slug) {
  const result = await rpc(actor.request, actor.token, "page_view", {
    site_id: siteId,
    session_token: actor.token,
    locales: ["en"],
    route: { slug, extra: "" }
  })
  assert.equal(result.type, "found")
  return result.data
}

/** @param {BrowserContext} context @param {string} passwordPath */
async function login(context, passwordPath) {
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  await page.locator('#login [name="nameOrEmail"]').fill("cobalt-import")
  await page
    .locator('#login [name="password"]')
    .fill((await readFile(passwordPath, "utf8")).trim())
  await page.locator('#login button[type="submit"]').click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find((item) => item.name === "wikijump_token")
  assert.ok(cookie, "login creates session cookie")
  const token = decodeURIComponent(cookie.value)
  const session = await rpc(context.request, token, "session_get", [token])
  assert.ok(Number.isSafeInteger(session?.user_id))
  return {
    page,
    actor: { request: context.request, token, actorId: Number(session.user_id) }
  }
}

/**
 * @param {Actor} actor @param {number} index @param
 *   {{tags?:string[],title?:string,category?:string,wikitext?:string}}
 *   [options] @returns {Promise<Fixture>}
 */
async function createFixture(actor, index, options = {}) {
  const slug = `${options.category ?? "local-tag-admin-proof"}:page-${index}-${nonce}`
  const tags = options.tags ?? (index ? [source, target, target, kept] : [source, kept])
  const created = await rpc(actor.request, actor.token, "page_import", {
    site_id: siteId,
    slug,
    title: options.title ?? `Tag admin fixture ${index}`,
    wikitext: options.wikitext ?? "Preserve this fixture body.",
    tags,
    user_id: actor.actorId,
    ip_address: "127.0.0.1",
    revision_comments: "Isolated tag admin fixture",
    layout: "wikidot"
  })
  return {
    slug,
    pageId: created.page_id,
    tags: tags.sort(),
    before: await readPage(actor, slug)
  }
}

/** @param {Actor} actor @param {Fixture[]} fixtures */
async function assertPreviewScope(actor, fixtures) {
  /** @type {TagPreview} */
  const snapshot = await rpc(actor.request, actor.token, "site_tag_preview", {
    source_tag: source,
    target_tag: target
  })
  assert.deepEqual(
    snapshot.pages.map((page) => page.page_id).sort((a, b) => a - b),
    fixtures.map((page) => page.pageId).sort((a, b) => a - b)
  )
}

/** @param {BrowserPage} page @param {'rename'|'delete'} operation */
async function previewChange(page, operation) {
  await page.locator("#tag-operation").selectOption(operation)
  await page.locator("#source-tag").fill(source)
  if (operation === "rename") await page.locator("#target-tag").fill(target)
  await page.getByRole("button", { name: "Preview affected pages" }).click()
  await expect(
    page.getByRole("heading", { name: "Preview: 2 pages", exact: true })
  ).toBeVisible()
}

/**
 * @param {Actor} actor @param {Fixture[]} fixtures @param
 *   {'original'|'merged'|'deleted'|'unchanged'} state
 */
async function assertFixtureState(actor, fixtures, state) {
  for (const fixture of fixtures) {
    const stored = await readPage(actor, fixture.slug)
    let expected = fixture.tags
    if (state === "merged") expected = [target, kept].sort()
    if (state === "deleted") expected = fixture.tags.filter((tag) => tag !== source)
    assert.deepEqual([...stored.page_revision.tags].sort(), expected)
    assert.deepEqual(
      stored.page_revision.wikitext_hash,
      fixture.before.page_revision.wikitext_hash
    )
    if (state === "unchanged") {
      assert.equal(stored.page.latest_revision_id, fixture.before.page.latest_revision_id)
    }
  }
}

/** @param {Actor} actor */
async function readPendingOperation(actor) {
  /** @type {TagOperation[]} */
  const history = await rpc(actor.request, actor.token, "site_tag_history", {})
  const operation = history.find((item) => item.source_tag === source && !item.reversed)
  assert.ok(operation)
  return operation
}

/** @param {BrowserPage} page @param {TagOperation} operation */
async function undoThroughUi(page, operation) {
  const article = page.locator("article").filter({
    has: page.getByRole("heading", {
      name: `#${operation.operation_id}: ${source} → ${target}`,
      exact: true
    })
  })
  await article.getByText("Undo this operation", { exact: true }).click()
  await article
    .getByRole("button", { name: `Confirm undo #${operation.operation_id}`, exact: true })
    .click()
  await expect(
    page.getByRole("status").filter({ hasText: "undone on 2 pages" })
  ).toBeVisible()
}

/** @param {Actor} actor @param {Fixture[]} fixtures */
async function deleteFixtures(actor, fixtures) {
  for (const fixture of fixtures) {
    assert.ok(
      /^local-tag-admin-proof(?:-other)?:/.test(fixture.slug) &&
        fixture.slug.endsWith(nonce)
    )
    const stored = await readPage(actor, fixture.slug)
    assert.equal(stored.page.page_id, fixture.pageId)
    await rpc(actor.request, actor.token, "page_delete", {
      site_id: siteId,
      page: fixture.pageId,
      last_revision_id: stored.page.latest_revision_id,
      revision_comments: "Remove isolated tag admin fixture",
      user_id: actor.actorId,
      ip_address: "127.0.0.1"
    })
  }
}

/** @param {import("@playwright/test").Browser} browser */
async function assertAnonymousDenied(browser) {
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
    try {
      const context = await browser.newContext()
      const { page, actor } = await login(context, passwordPath)
      /** @type {Fixture[]} */
      const fixtures = []
      try {
        fixtures.push(await createFixture(actor, 0))
        fixtures.push(await createFixture(actor, 1))
        await assertPreviewScope(actor, fixtures)
        await page.goto(`${origin}/-/admin/tags`, { waitUntil: "networkidle" })
        await expect(
          page.getByRole("heading", { name: "Manage tags", exact: true })
        ).toBeVisible()
        await previewChange(page, "rename")
        await page.getByRole("button", { name: "Cancel", exact: true }).click()
        await expect(
          page.getByRole("button", { name: "Confirm rename", exact: true })
        ).toHaveCount(0)
        await assertFixtureState(actor, fixtures, "unchanged")

        await previewChange(page, "rename")
        await page.getByRole("button", { name: "Confirm rename", exact: true }).click()
        await expect(
          page.getByRole("status").filter({ hasText: "Tags changed on 2 pages" })
        ).toBeVisible()
        await assertFixtureState(actor, fixtures, "merged")
        await undoThroughUi(page, await readPendingOperation(actor))
        await assertFixtureState(actor, fixtures, "original")

        await previewChange(page, "delete")
        await page
          .getByRole("button", { name: "Confirm delete tag", exact: true })
          .click()
        await expect(
          page.getByRole("status").filter({ hasText: "Tags changed on 2 pages" })
        ).toBeVisible()
        await assertFixtureState(actor, fixtures, "deleted")
        const deleted = await readPendingOperation(actor)
        await rpc(actor.request, actor.token, "site_tag_undo", {
          operation_id: deleted.operation_id,
          ip_address: "127.0.0.1"
        })
        await assertFixtureState(actor, fixtures, "original")
        await assertAnonymousDenied(browser)
      } finally {
        await deleteFixtures(actor, fixtures)
      }
    } finally {
      await browser.close()
    }
  }
)

/**
 * @param {Actor} actor @param {Fixture[]} fixtures @param {boolean}
 *   applied
 */
async function assertAddedState(actor, fixtures, applied) {
  for (const [index, fixture] of fixtures.entries()) {
    const stored = await readPage(actor, fixture.slug)
    const expected =
      applied && index < 2 ? [...fixture.tags, target].sort() : fixture.tags
    assert.deepEqual([...stored.page_revision.tags].sort(), expected)
    assert.deepEqual(
      stored.page_revision.wikitext_hash,
      fixture.before.page_revision.wikitext_hash
    )
    if (index >= 2) {
      assert.equal(
        stored.page.latest_revision_id,
        fixture.before.page.latest_revision_id,
        "excluded and already-tagged pages have no new revision"
      )
    }
  }
}

/** @param {Actor} actor @param {Fixture[]} fixtures */
async function createAddFixtures(actor, fixtures) {
  const variants = [
    { title: "Ready 100%_ One", tags: [source, kept] },
    { title: "READY 100%_ Two", tags: [source, kept] },
    { title: "Ready 100%_ Already tagged", tags: [source, target, kept] },
    {
      title: "Different title",
      tags: [source, kept],
      wikitext: "Ready 100%_ in body only"
    },
    {
      title: "Ready 100%_ Other category",
      tags: [source, kept],
      category: "local-tag-admin-proof-other"
    },
    { title: "Ready 100%_ Missing source tag", tags: [kept] },
    { title: "Ready 100xyz_ Wildcard lookalike", tags: [source, kept] }
  ]
  for (const [index, variant] of variants.entries()) {
    fixtures.push(await createFixture(actor, index + 10, variant))
  }
}

/** @param {BrowserPage} page @param {TagOperation} operation */
async function undoAddition(page, operation) {
  const article = page.locator("article").filter({
    has: page.getByRole("heading", {
      name: `#${operation.operation_id}: Add ${target}`,
      exact: true
    })
  })
  await article.getByText("Undo this operation", { exact: true }).click()
  await article
    .getByRole("button", { name: `Confirm undo #${operation.operation_id}`, exact: true })
    .click()
  await expect(
    page.getByRole("status").filter({ hasText: "undone on 2 pages" })
  ).toBeVisible()
}

test(
  "bulk-add combines title-only, category and tag filters, skips existing tags and undoes",
  { timeout: 180000 },
  async () => {
    const passwordPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
    assert.ok(passwordPath, "local admin password file required")
    const browser = await chromium.launch({
      executablePath: "/usr/bin/chromium",
      headless: true
    })
    try {
      const context = await browser.newContext()
      const { page, actor } = await login(context, passwordPath)
      /** @type {Fixture[]} */
      const fixtures = []
      try {
        await createAddFixtures(actor, fixtures)
        await page.goto(`${origin}/-/admin/tags`, { waitUntil: "networkidle" })
        await page.locator("#tag-operation").selectOption("add")
        await page.locator("#add-tag").fill(target)
        await page.locator("#filter-tag").fill(source)
        await page.locator("#filter-category").fill("local-tag-admin-proof")
        await page.locator("#filter-title").fill("ready 100%_")
        await page.getByRole("button", { name: "Preview affected pages" }).click()
        const previewSection = page.locator('section[aria-labelledby="preview-heading"]')
        await expect(
          previewSection.getByRole("heading", { name: "Preview: 2 pages", exact: true })
        ).toBeVisible()
        assert.deepEqual(
          await previewSection
            .locator("li a")
            .evaluateAll((links) =>
              links.map((link) => link.getAttribute("href")).sort()
            ),
          fixtures
            .slice(0, 2)
            .map((f) => `/${f.slug}`)
            .sort()
        )
        await assertAddedState(actor, fixtures, false)
        await page.getByRole("button", { name: "Confirm add tag", exact: true }).click()
        await expect(
          page.getByRole("status").filter({ hasText: "Tags changed on 2 pages" })
        ).toBeVisible()
        await assertAddedState(actor, fixtures, true)
        const operation = await readPendingOperationForAdd(actor)
        assert.equal(operation.kind, "add")
        assert.deepEqual(operation.filters, {
          existing_tag: source,
          category: "local-tag-admin-proof",
          title_contains: "ready 100%_"
        })
        await undoAddition(page, operation)
        await assertAddedState(actor, fixtures, false)
      } finally {
        await deleteFixtures(actor, fixtures)
      }
    } finally {
      await browser.close()
    }
  }
)

/** @param {Actor} actor */
async function readPendingOperationForAdd(actor) {
  /** @type {TagOperation[]} */
  const history = await rpc(actor.request, actor.token, "site_tag_history", {})
  const operation = history.find(
    (item) => item.kind === "add" && item.target_tag === target && !item.reversed
  )
  assert.ok(operation)
  return operation
}
