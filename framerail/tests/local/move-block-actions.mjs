import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"
import { decodeMutation, origin, rpc, siteId } from "./file-action-transport.mjs"

const fixturePath =
  "/home/osso/.local/share/cobalt-wiki/local-full/page-action-fixture.json"
const sourceId = 3000006134
const destinationId = 3000006135
const linkLabels = ["First", "Second", "Third"]

/** @typedef {import("../../src/lib/server/deepwell/views").PageView} PageView */
/** @typedef {Extract<PageView, { type: "found" }>["data"]} StoredPage */
/** @typedef {ReturnType<typeof snapshot>} PageSnapshot */
/** @typedef {Awaited<ReturnType<typeof login>>} Actor */
/** @typedef {Awaited<ReturnType<typeof guardBrowserWrites>>} WriteGuard */
/**
 * @typedef {{
 *   sacrificial: true
 *   siteId: number
 *   siteSlug: string
 *   databaseLabel: string
 *   username: string
 *   sourceSlug: string
 *   movedSlug: string
 *   destinationSlug: string
 *   pageId: number
 *   destinationPageId: number
 *   created: { slug: string; pageId: number }[]
 * }} Fixture
 */

/**
 * @typedef {{
 *   source: PageSnapshot
 *   destination: PageSnapshot
 *   blocked: boolean
 *   destinationRevision: number
 * }} Baseline
 */

async function readFixture() {
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(fixturePath, "utf8"))
  assert.equal(fixture.sacrificial, true)
  assert.equal(fixture.siteId, siteId)
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.username, "cobalt-import")
  const match = /^local-action-proof:source-([a-f0-9]{16})$/.exec(fixture.sourceSlug)
  assert.ok(match, "sacrificial source slug required")
  assert.equal(fixture.movedSlug, `local-action-proof:moved-${match[1]}`)
  assert.equal(fixture.destinationSlug, `local-action-proof:destination-${match[1]}`)
  assert.equal(fixture.pageId, sourceId)
  assert.equal(fixture.destinationPageId, destinationId)
  assert.deepEqual(
    fixture.created.map(({ slug, pageId }) => ({ slug, pageId })),
    [
      { slug: fixture.sourceSlug, pageId: sourceId },
      { slug: fixture.destinationSlug, pageId: destinationId }
    ]
  )
  return fixture
}

/** @param {string} slug */
function sourceLinks(slug) {
  return linkLabels.map((label) => `[[[${slug}|${label}]]]`).join(" ")
}

/** @param {StoredPage} page */
function snapshot(page) {
  return {
    slug: page.page.slug,
    wikitext: page.wikitext,
    title: page.page_revision.title,
    altTitle: page.page_revision.alt_title,
    tags: page.page_revision.tags,
    layout: page.page.layout
  }
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {string} slug
 * @param {number} id
 * @returns {Promise<StoredPage>}
 */
async function readOwned(request, token, slug, id) {
  /** @type {PageView} */
  const view = await rpc(request, token, slug, "page_view", {
    site_id: siteId,
    locales: ["en"],
    session_token: token,
    route: { slug, extra: "" }
  })
  assert.ok(view.type === "found", `${slug} must exist`)
  assert.equal(view.data.page.page_id, id, "only manifest page ID allowed")
  assert.equal(view.data.page.slug, slug)
  return view.data
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {string} slug
 */
async function assertVacant(request, token, slug) {
  /** @type {PageView} */
  const view = await rpc(request, token, slug, "page_view", {
    site_id: siteId,
    locales: ["en"],
    session_token: token,
    route: { slug, extra: "" }
  })
  assert.equal(view.type, "missing", `${slug} must be vacant`)
}

/**
 * @param {Actor} actor
 * @param {string} slug
 * @param {number} id
 * @param {StoredPage} current
 * @param {Partial<
 *   Pick<StoredPage["page_revision"], "title" | "alt_title" | "tags"> &
 *     Pick<StoredPage, "wikitext">
 * >} fields
 */
async function fixtureEdit(actor, slug, id, current, fields) {
  assert.ok([sourceId, destinationId].includes(id))
  assert.equal(current.page.page_id, id)
  assert.equal(current.page.slug, slug)
  const result = await rpc(actor.request, actor.token, slug, "page_edit", {
    site_id: siteId,
    page: id,
    user_id: actor.userId,
    ip_address: "127.0.0.1",
    last_revision_id: current.page_revision.revision_id,
    revision_comments: "Disposable Move UI fixture",
    ...fields
  })
  assert.ok(result.revision_id > current.page_revision.revision_id)
}

/**
 * @param {Actor} actor @param {Fixture} fixture @param {string} from
 *   @param {string} to
 */
async function fixtureMove(actor, fixture, from, to) {
  assert.ok([fixture.sourceSlug, fixture.movedSlug].includes(from))
  assert.ok([fixture.sourceSlug, fixture.movedSlug].includes(to))
  const current = await readOwned(actor.request, actor.token, from, sourceId)
  await assertVacant(actor.request, actor.token, to)
  await rpc(actor.request, actor.token, from, "page_move", {
    site_id: siteId,
    page: sourceId,
    user_id: actor.userId,
    ip_address: "127.0.0.1",
    last_revision_id: current.page_revision.revision_id,
    new_slug: to,
    fix_dependencies: [],
    revision_comments: "Restore disposable Move UI fixture slug"
  })
}

/**
 * @param {Actor} actor @param {string} slug @returns {Promise<{ blocked:
 *   boolean }>}
 */
async function readBlock(actor, slug) {
  const page = await readOwned(actor.request, actor.token, slug, sourceId)
  assert.equal(page.page.page_id, sourceId)
  return rpc(actor.request, actor.token, slug, "page_block_get", { page: sourceId })
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {Fixture} fixture
 * @param {string} password
 */
async function login(context, fixture, password) {
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  await expect(page.locator("#login")).toBeVisible()
  await page.locator('#login [name="nameOrEmail"]').fill(fixture.username)
  await page.locator('#login [name="password"]').fill(password)
  await page.locator('#login button[type="submit"]').click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (entry) => entry.name === "wikijump_token" && entry.domain === "127.0.0.1"
  )
  assert.ok(cookie, "authenticated admin cookie required")
  const token = decodeURIComponent(cookie.value)
  /** @type {import("../../src/lib/types").SessionModel} */
  const session = await rpc(context.request, token, fixture.sourceSlug, "session_get", [
    token
  ])
  assert.ok(Number.isSafeInteger(session.user_id))
  return { page, request: context.request, token, userId: session.user_id }
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {Fixture} fixture
 */
async function guardBrowserWrites(context, fixture) {
  /** @type {string[]} */
  const blocked = []
  /** @type {string[]} */
  const writes = []
  /**
   * @type {{
   *   slug: string
   *   action: "move" | "blockSet"
   *   blocked: boolean | null
   * } | null}
   */
  let permitted = null
  await context.route("**/*", async (route) => {
    const request = route.request()
    const url = new URL(request.url())
    const stylesheet =
      url.origin === "https://d3g0gp89917ko0.cloudfront.net" &&
      request.method() === "GET" &&
      request.resourceType() === "stylesheet"
    if (stylesheet) return route.continue()
    if (url.origin !== origin) {
      if (!["GET", "HEAD"].includes(request.method()))
        blocked.push(`foreign ${request.method()} ${url.origin}`)
      return route.abort()
    }
    if (["GET", "HEAD", "OPTIONS"].includes(request.method())) return route.continue()
    const path = `${url.pathname}${url.search}`
    const loginPost =
      request.method() === "POST" && ["/-/login", "/-/login?/login"].includes(path)
    const readPost =
      request.method() === "POST" &&
      ["backlinks", "blockGet"].some(
        (action) =>
          path === `/${fixture.sourceSlug}?/${action}` ||
          path === `/${fixture.movedSlug}?/${action}`
      )
    if (loginPost || readPost) return route.continue()
    if (
      request.method() === "POST" &&
      permitted &&
      path === `/${permitted.slug}?/${permitted.action}`
    ) {
      try {
        const { data: body } = await decodeMutation(request)
        assert.equal(body.pageId, sourceId)
        if (permitted.action === "move") {
          assert.equal(body.siteId, siteId)
          assert.equal(body.newSlug, fixture.movedSlug)
          assert.deepEqual(body.fixDependencies, [destinationId])
        } else {
          assert.equal(body.blocked, permitted.blocked)
        }
        writes.push(path)
        permitted = null
        return route.continue()
      } catch (error) {
        blocked.push(`${path}: ${String(error)}`)
        return route.abort()
      }
    }
    blocked.push(`${request.method()} ${path}`)
    return route.abort()
  })
  return {
    writes,
    blocked,
    /**
     * @param {string} slug @param {"move" | "blockSet"} action @param
     *   {boolean | null} [blockedState]
     */
    allow(slug, action, blockedState = null) {
      assert.equal(permitted, null, "previous UI write not consumed")
      assert.ok([fixture.sourceSlug, fixture.movedSlug].includes(slug))
      assert.ok(action === "move" || action === "blockSet")
      permitted = { slug, action, blocked: blockedState }
    },
    assertConsumed() {
      assert.equal(permitted, null, "expected UI mutation not dispatched")
      assert.deepEqual(blocked, [], "unexpected browser writes blocked")
    }
  }
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {WriteGuard} guard
 * @param {string} slug
 * @param {"move" | "blockSet"} action
 * @param {import("@playwright/test").Locator} button
 * @param {boolean | null} [blocked]
 */
async function submitAction(page, guard, slug, action, button, blocked = null) {
  const before = guard.writes.length
  const responsePromise = page.waitForResponse(
    (response) =>
      response.url() === `${origin}/${slug}?/${action}` &&
      response.request().method() === "POST"
  )
  guard.allow(slug, action, blocked)
  const [response] = await Promise.all([responsePromise, button.click()])
  assert.equal(response.status(), 200, `${action} HTTP status`)
  const result = await response.json()
  assert.equal(result.type, "success", `${action} failed: ${result.status}`)
  assert.equal(guard.writes.length, before + 1, `${action} write count`)
  guard.assertConsumed()
}

/** @param {import("@playwright/test").Page} page @param {string} slug */
async function openOptions(page, slug) {
  const response = await page.goto(`${origin}/${slug}`, { waitUntil: "networkidle" })
  assert.equal(response?.status(), 200)
  const options = page.locator("#page-options-bottom-2")
  if (!(await options.isVisible())) await page.locator("#more-options-button").click()
  await expect(options).toBeVisible()
  return options
}

/**
 * @param {Actor} actor @param {Fixture} fixture @param {WriteGuard} guard
 *   @param {Baseline} baseline
 */
async function proveMove(actor, fixture, guard, baseline) {
  const options = await openOptions(actor.page, fixture.sourceSlug)
  await options.locator("#rename-move-button").click()
  const form = actor.page.locator("#page-move")
  await expect(form).toBeVisible()
  await form.getByRole("button", { name: "Show dependencies" }).click()
  const choices = actor.page.getByRole("region", { name: "Dependency repair choices" })
  await expect(choices).toBeVisible()
  const destination = choices.locator(`input[type="checkbox"][value="${destinationId}"]`)
  const source = choices.locator(`input[type="checkbox"][value="${sourceId}"]`)
  await expect(destination).toHaveCount(1)
  await expect(source).toHaveCount(1)
  await destination.check()
  await expect(source).not.toBeChecked()
  await form.locator('[name="new-slug"]').fill(fixture.movedSlug)
  await submitAction(
    actor.page,
    guard,
    fixture.sourceSlug,
    "move",
    form.locator('[type="submit"]')
  )
  const remaining = actor.page.getByRole("region", {
    name: "Dependencies remaining after move"
  })
  await expect(
    actor.page.getByText("Page moved, but some dependencies remain.")
  ).toBeVisible()
  await expect(remaining.getByRole("heading", { name: "Links" })).toBeVisible()
  await expect(
    remaining.getByRole("link", {
      name: `${baseline.source.title || fixture.movedSlug} (${fixture.movedSlug})`
    })
  ).toHaveAttribute("href", `/${encodeURIComponent(fixture.movedSlug)}`)
  await expect(remaining.getByRole("heading", { name: "Inclusions" })).toBeVisible()
  await expect(remaining.getByText("No inclusions remain.")).toBeVisible()
  await assertVacant(actor.request, actor.token, fixture.sourceSlug)
  const moved = await readOwned(actor.request, actor.token, fixture.movedSlug, sourceId)
  const destinationAfter = await readOwned(
    actor.request,
    actor.token,
    fixture.destinationSlug,
    destinationId
  )
  assert.equal(
    moved.wikitext,
    `[[[${fixture.sourceSlug}|Self]]]`,
    "unselected self-link persists"
  )
  assert.equal(
    destinationAfter.wikitext,
    sourceLinks(fixture.movedSlug),
    "all three labels and new targets"
  )
  assert.ok(destinationAfter.page_revision.revision_id > baseline.destinationRevision)
  assert.equal(
    destinationAfter.page_revision.user_id,
    actor.userId,
    "repair revision actor"
  )
  await actor.page.getByRole("button", { name: "Continue to new page" }).click()
  await expect(actor.page).toHaveURL(`${origin}/${fixture.movedSlug}`)
}

/** @param {Actor} actor @param {Fixture} fixture @param {WriteGuard} guard */
async function proveBlock(actor, fixture, guard) {
  const options = await openOptions(actor.page, fixture.movedSlug)
  await options.locator("#lock-page-button").click()
  const checkbox = actor.page.locator("#page-block-checkbox")
  await expect(checkbox).toBeEnabled()
  await expect(checkbox).not.toBeChecked()
  await checkbox.check()
  await submitAction(
    actor.page,
    guard,
    fixture.movedSlug,
    "blockSet",
    actor.page.getByRole("button", { name: "Save" }),
    true
  )
  await expect(checkbox).toHaveCount(0)
  assert.equal((await readBlock(actor, fixture.movedSlug)).blocked, true)
  const reloaded = await openOptions(actor.page, fixture.movedSlug)
  await reloaded.locator("#lock-page-button").click()
  await expect(checkbox).toBeEnabled()
  await expect(checkbox).toBeChecked()
  await checkbox.uncheck()
  await submitAction(
    actor.page,
    guard,
    fixture.movedSlug,
    "blockSet",
    actor.page.getByRole("button", { name: "Save" }),
    false
  )
  await expect(checkbox).toHaveCount(0)
  assert.equal((await readBlock(actor, fixture.movedSlug)).blocked, false)
  const final = await openOptions(actor.page, fixture.movedSlug)
  await final.locator("#lock-page-button").click()
  await expect(checkbox).toBeEnabled()
  await expect(checkbox).not.toBeChecked()
}

/**
 * @param {Actor} actor
 * @param {string} slug
 * @param {number} id
 * @param {PageSnapshot} baseline
 * @param {string[]} expected
 */
async function restoreContent(actor, slug, id, baseline, expected) {
  const current = await readOwned(actor.request, actor.token, slug, id)
  assert.ok(
    expected.includes(current.wikitext),
    `refuse recovery of unexpected ${slug} source`
  )
  if (
    current.wikitext !== baseline.wikitext ||
    current.page_revision.title !== baseline.title ||
    JSON.stringify(current.page_revision.tags) !== JSON.stringify(baseline.tags) ||
    current.page_revision.alt_title !== baseline.altTitle
  ) {
    await fixtureEdit(actor, slug, id, current, {
      wikitext: baseline.wikitext,
      title: baseline.title,
      alt_title: baseline.altTitle,
      tags: baseline.tags
    })
  }
  const updated = await readOwned(actor.request, actor.token, slug, id)
  if (updated.page.layout !== baseline.layout) {
    await rpc(actor.request, actor.token, slug, "page_set_layout", {
      site_id: siteId,
      page_id: id,
      user_id: actor.userId,
      ip_address: "127.0.0.1",
      layout: baseline.layout
    })
  }
}

/** @param {Actor} actor @param {Fixture} fixture @param {Baseline} baseline */
async function recover(actor, fixture, baseline) {
  /** @type {PageView} */
  const sourceView = await rpc(
    actor.request,
    actor.token,
    fixture.sourceSlug,
    "page_view",
    {
      site_id: siteId,
      locales: ["en"],
      session_token: actor.token,
      route: { slug: fixture.sourceSlug, extra: "" }
    }
  )
  if (sourceView.type === "missing") {
    const moved = await readOwned(actor.request, actor.token, fixture.movedSlug, sourceId)
    assert.equal(moved.wikitext, `[[[${fixture.sourceSlug}|Self]]]`)
    await fixtureMove(actor, fixture, fixture.movedSlug, fixture.sourceSlug)
  } else {
    assert.ok(sourceView.type === "found", "source fixture must exist")
    assert.equal(sourceView.data.page.page_id, sourceId)
    await assertVacant(actor.request, actor.token, fixture.movedSlug)
  }
  await restoreContent(actor, fixture.sourceSlug, sourceId, baseline.source, [
    baseline.source.wikitext,
    `[[[${fixture.sourceSlug}|Self]]]`
  ])
  await restoreContent(
    actor,
    fixture.destinationSlug,
    destinationId,
    baseline.destination,
    [
      baseline.destination.wikitext,
      sourceLinks(fixture.sourceSlug),
      sourceLinks(fixture.movedSlug)
    ]
  )
  const block = await readBlock(actor, fixture.sourceSlug)
  if (block.blocked !== baseline.blocked) {
    await rpc(actor.request, actor.token, fixture.sourceSlug, "page_block_set", {
      page: sourceId,
      blocked: baseline.blocked,
      ip_address: "127.0.0.1"
    })
  }
  assert.deepEqual(
    snapshot(await readOwned(actor.request, actor.token, fixture.sourceSlug, sourceId)),
    baseline.source
  )
  assert.deepEqual(
    snapshot(
      await readOwned(actor.request, actor.token, fixture.destinationSlug, destinationId)
    ),
    baseline.destination
  )
  assert.equal((await readBlock(actor, fixture.sourceSlug)).blocked, baseline.blocked)
  await assertVacant(actor.request, actor.token, fixture.movedSlug)
}

/** @param {Actor} actor @param {Fixture} fixture @param {WriteGuard} guard */
async function exercise(actor, fixture, guard) {
  const source = await readOwned(actor.request, actor.token, fixture.sourceSlug, sourceId)
  const destination = await readOwned(
    actor.request,
    actor.token,
    fixture.destinationSlug,
    destinationId
  )
  await assertVacant(actor.request, actor.token, fixture.movedSlug)
  const block = await readBlock(actor, fixture.sourceSlug)
  assert.equal(
    block.blocked,
    false,
    "fixture must start unblocked; refuse to clear preexisting Block"
  )
  const baseline = {
    source: snapshot(source),
    destination: snapshot(destination),
    blocked: block.blocked,
    destinationRevision: destination.page_revision.revision_id
  }
  /** @type {unknown} */
  let failure = null
  try {
    await fixtureEdit(actor, fixture.sourceSlug, sourceId, source, {
      wikitext: `[[[${fixture.sourceSlug}|Self]]]`
    })
    await fixtureEdit(actor, fixture.destinationSlug, destinationId, destination, {
      wikitext: sourceLinks(fixture.sourceSlug)
    })
    if (source.page.layout !== "wikidot") {
      await readOwned(actor.request, actor.token, fixture.sourceSlug, sourceId)
      await rpc(actor.request, actor.token, fixture.sourceSlug, "page_set_layout", {
        site_id: siteId,
        page_id: sourceId,
        user_id: actor.userId,
        ip_address: "127.0.0.1",
        layout: "wikidot"
      })
    }
    await proveMove(actor, fixture, guard, baseline)
    await proveBlock(actor, fixture, guard)
    assert.deepEqual(guard.writes, [
      `/${fixture.sourceSlug}?/move`,
      `/${fixture.movedSlug}?/blockSet`,
      `/${fixture.movedSlug}?/blockSet`
    ])
  } catch (error) {
    failure = error
  } finally {
    try {
      await recover(actor, fixture, baseline)
    } catch (recoveryError) {
      failure = failure
        ? new AggregateError(
            [failure, recoveryError],
            "UI assertion failed; fixture recovery also failed"
          )
        : recoveryError
    }
  }
  if (failure) throw failure
}

test(
  "Move repairs only selected links, displays leftovers; Wikidot Block persists set and clear",
  { timeout: 120_000 },
  async () => {
    const adminPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
    const gatewayPath = process.env.COBALT_LOCAL_PASSWORD_FILE
    assert.ok(
      adminPath && gatewayPath && process.env.COBALT_PAGE_ACTION_FIXTURE === fixturePath,
      "explicit fixture and admin/gateway password paths required"
    )
    const fixture = await readFixture()
    const browser = await chromium.launch({
      executablePath: "/usr/bin/chromium",
      headless: true
    })
    try {
      const context = await browser.newContext({
        httpCredentials: {
          username: "cobalt",
          password: (await readFile(gatewayPath, "utf8")).trim(),
          origin
        }
      })
      try {
        const guard = await guardBrowserWrites(context, fixture)
        const actor = await login(
          context,
          fixture,
          (await readFile(adminPath, "utf8")).trim()
        )
        await exercise(actor, fixture, guard)
        guard.assertConsumed()
      } finally {
        await context.close()
      }
    } finally {
      await browser.close()
    }
  }
)
