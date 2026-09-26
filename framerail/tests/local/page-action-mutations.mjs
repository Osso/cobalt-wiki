import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { open, readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"
const { parse } = await import(
  new URL("../../node_modules/.pnpm/node_modules/devalue/index.js", import.meta.url).href
)

const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000

/** @typedef {import("../../src/lib/server/deepwell/views").PageView} PageView */
/** @typedef {Extract<PageView, { type: "found" }>["data"]} StoredPage */
/** @typedef {Awaited<ReturnType<typeof guardBrowserWrites>>} WriteGuard */
/** @typedef {import("../../src/lib/server/deepwell/page").PageDeletedGet} PageDeletedGet */
/** @typedef {import("../../src/lib/types").SessionModel} SessionModel */
/**
 * @typedef {{
 *   sacrificial: true
 *   siteId: 6000000
 *   siteSlug: "cobalt-company"
 *   databaseLabel: "cobalt_local_full"
 *   username: "cobalt-import"
 *   sourceSlug: string
 *   movedSlug: string
 *   destinationSlug: string
 *   pageId: number
 *   destinationPageId: number
 *   created: { slug: string; pageId: number; revisionId: number }[]
 * }} Fixture
 */

/** @param {string} source */
function hash(source) {
  return createHash("sha256").update(source).digest("hex")
}

/** @param {string} path */
async function readFixture(path) {
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(path, "utf8"))
  assert.equal(fixture.sacrificial, true, "explicit disposable fixture required")
  assert.equal(fixture.siteId, siteId)
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.username, "cobalt-import")
  const match = /^local-action-proof:source-([a-f0-9]{16})$/.exec(fixture.sourceSlug)
  if (!match) assert.fail("unique disposable source slug required")
  assert.equal(fixture.movedSlug, `local-action-proof:moved-${match[1]}`)
  assert.equal(fixture.destinationSlug, `local-action-proof:destination-${match[1]}`)
  assert.ok(Number.isSafeInteger(fixture.pageId) && fixture.pageId > 0)
  assert.ok(
    Number.isSafeInteger(fixture.destinationPageId) && fixture.destinationPageId > 0
  )
  assert.notEqual(fixture.pageId, fixture.destinationPageId)
  assert.ok(Array.isArray(fixture.created), "created page manifest required")
  assert.deepEqual(
    fixture.created.map(({ slug, pageId }) => ({ slug, pageId })),
    [
      { slug: fixture.sourceSlug, pageId: fixture.pageId },
      { slug: fixture.destinationSlug, pageId: fixture.destinationPageId }
    ]
  )
  for (const created of fixture.created) {
    assert.ok(
      Number.isSafeInteger(created.revisionId) && created.revisionId > 0,
      `created revision required for ${created.slug}`
    )
  }
  return fixture
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {string} pageRef
 * @param {string} method
 * @param {unknown} params
 */
async function rpc(request, token, pageRef, method, params) {
  const response = await request.post(backend, {
    headers: {
      "X-Deepwell-Site-Id": String(siteId),
      "X-Deepwell-Page": pageRef,
      "X-Deepwell-Session-Token": token
    },
    data: { jsonrpc: "2.0", id: 1, method, params }
  })
  assert.equal(response.status(), 200, `${method} HTTP status`)
  const payload = await response.json()
  assert.ok(!payload.error, `${method} RPC error ${payload.error?.code ?? "?"}`)
  return payload.result
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {string} slug
 * @returns {Promise<PageView>}
 */
async function readPage(request, token, slug) {
  return rpc(request, token, slug, "page_view", {
    site_id: siteId,
    locales: ["en"],
    session_token: token,
    route: { slug, extra: "" }
  })
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {Fixture} fixture
 * @param {string} slug
 * @param {string} sourceHash
 * @returns {Promise<StoredPage>}
 */
async function assertOwnedPage(request, token, fixture, slug, sourceHash) {
  const view = await readPage(request, token, slug)
  if (view.type !== "found") assert.fail(`fixture ${slug} must be live`)
  assert.equal(view.data.page.page_id, fixture.pageId, "only manifest page ID allowed")
  assert.equal(view.data.page.slug, slug)
  assert.equal(
    hash(view.data.wikitext),
    sourceHash,
    "fixture source hash must not change"
  )
  return view.data
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {string} slug
 */
async function assertMissing(request, token, slug) {
  assert.equal(
    (await readPage(request, token, slug))?.type,
    "missing",
    `${slug} must be empty`
  )
}

/**
 * @param {import("@playwright/test").BrowserContext} context
 * @param {Fixture} fixture
 */
async function guardBrowserWrites(context, fixture) {
  /** @type {{ slug: string; action: string } | null} */
  let permitted = null
  /** @type {string[]} */
  const blocked = []
  /** @type {{ method: string; origin: string; resourceType: string }[]} */
  const externalReads = []
  /** @type {string[]} */
  const writes = []
  await context.route("**/*", (route) => {
    const request = route.request()
    const url = new URL(request.url())
    const isBaseStylesheet =
      url.origin === "https://d3g0gp89917ko0.cloudfront.net" &&
      request.method() === "GET" &&
      request.resourceType() === "stylesheet"
    if (isBaseStylesheet) return route.continue()
    if (url.origin !== origin) {
      if (["GET", "HEAD"].includes(request.method())) {
        externalReads.push({
          method: request.method(),
          origin: url.origin,
          resourceType: request.resourceType()
        })
      } else {
        blocked.push(`${request.method()} foreign origin ${url.origin}`)
      }
      return route.abort()
    }
    if (["GET", "HEAD", "OPTIONS"].includes(request.method())) return route.continue()
    const path = `${url.pathname}${url.search}`
    const login = path === "/-/login" || path === "/-/login?/login"
    const deletedList = path === `/${fixture.movedSlug}?/deletedGet`
    const mutation = permitted && path === `/${permitted.slug}?/${permitted.action}`
    if (request.method() === "POST" && (login || deletedList || mutation)) {
      if (mutation) {
        writes.push(path)
        permitted = null
      }
      return route.continue()
    }
    blocked.push(`${request.method()} ${path}`)
    return route.abort()
  })
  return {
    blocked,
    externalReads,
    writes,
    /** @param {string} slug @param {string} action */
    allow(slug, action) {
      assert.equal(permitted, null, "previous write must be consumed")
      permitted = { slug, action }
    },
    assertConsumed() {
      assert.equal(permitted, null, "expected UI POST was not sent")
      assert.deepEqual(blocked, [], "unexpected browser requests must be blocked")
    }
  }
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
  if (!cookie) assert.fail("real browser login must establish session")
  return { page, token: decodeURIComponent(cookie.value) }
}

/** @param {import("@playwright/test").Page} page @param {string} slug */
async function visit(page, slug) {
  const response = await page.goto(`${origin}/${slug}`, { waitUntil: "networkidle" })
  assert.equal(response?.status(), 200, `${slug} HTTP status`)
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {string} wikidotButton
 * @param {string} wikijumpButton
 */
async function openPane(page, wikidotButton, wikijumpButton) {
  const moreOptions = page.locator("#more-options-button")
  if (await moreOptions.isVisible()) {
    const options = page.locator("#page-options-bottom-2")
    if (!(await options.isVisible())) await moreOptions.click()
    await expect(options).toBeVisible()
    await options.locator(`#${wikidotButton}`).click()
    return
  }
  await page.locator(`.editor-actions .${wikijumpButton}`).click()
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {WriteGuard} guard
 * @param {string} from
 * @param {string} to
 */
async function moveInBrowser(page, guard, from, to) {
  await visit(page, from)
  await openPane(page, "rename-move-button", "button-move")
  const form = page.locator("#page-move")
  await expect(form).toBeVisible()
  await form.locator('[name="new-slug"]').fill(to)
  guard.allow(from, "move")
  await form.locator('[type="submit"]').click()
  await expect(page).toHaveURL(`${origin}/${to}`)
  guard.assertConsumed()
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {WriteGuard} guard
 * @param {string} slug
 * @param {string | null} layout
 */
async function selectLayout(page, guard, slug, layout) {
  await visit(page, slug)
  await openPane(page, "layout-button", "button-layout")
  const form = page.locator("#page-layout")
  await expect(form).toBeVisible()
  await form
    .locator("select.page-layout-select")
    .selectOption(layout === null ? "" : layout)
  guard.allow(slug, "layout")
  await form.locator('[type="submit"]').click()
  await expect(form).toHaveCount(0)
  guard.assertConsumed()
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {WriteGuard} guard
 * @param {string} slug
 */
async function deleteInBrowser(page, guard, slug) {
  await visit(page, slug)
  await openPane(page, "delete-button", "button-delete")
  const form = page.locator("#page-delete")
  await expect(form).toBeVisible()
  await form.locator("#page-delete-option-delete").check()
  await form.locator('[type="submit"]').click()
  const dialog = page.getByRole("dialog", { name: "Delete page?" })
  await expect(dialog).toBeVisible()
  const responsePromise = page.waitForResponse(
    (response) =>
      response.url() === `${origin}/${slug}?/delete` &&
      response.request().method() === "POST"
  )
  guard.allow(slug, "delete")
  await dialog.getByRole("button", { name: "Delete page" }).click()
  const response = await responsePromise
  assert.equal(response.status(), 200, "Delete HTTP status")
  const result = await response.json()
  assert.equal(result.type, "success", `Delete action failed: ${result.status}`)
  await expect(form).toHaveCount(0)
  guard.assertConsumed()
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {Fixture} fixture
 */
async function assertDeleted(request, token, fixture) {
  await assertMissing(request, token, fixture.movedSlug)
  /** @type {PageDeletedGet[]} */
  const deleted = await rpc(request, token, fixture.movedSlug, "page_get_deleted", {
    site_id: siteId,
    slug: fixture.movedSlug
  })
  assert.ok(
    deleted.some(
      (item) => item.page_id === fixture.pageId && item.slug === fixture.movedSlug
    ),
    "only manifest page may be restored"
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {WriteGuard} guard
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {Fixture} fixture
 */
async function restoreInBrowser(page, guard, request, token, fixture) {
  await assertDeleted(request, token, fixture)
  const response = await page.goto(`${origin}/${fixture.movedSlug}`, {
    waitUntil: "networkidle"
  })
  assert.equal(response?.status(), 404, "deleted-page Restore view HTTP status")
  const restoreButton = page.locator("#restore-button, .editor-button.button-restore")
  if (!(await restoreButton.isVisible())) {
    assert.fail(
      "deleted-page UI restore unavailable; no RPC substitute counted as UI proof"
    )
  }
  await restoreButton.click()
  const form = page.locator("#page-restore")
  await expect(form).toBeVisible()
  const choice = form.locator(`#restore-page-id-${fixture.pageId}`)
  await expect(choice).toHaveCount(1)
  await choice.check()
  assert.equal(
    await form.locator('input[name="pageId"]:checked').inputValue(),
    String(fixture.pageId)
  )
  const responsePromise = page.waitForResponse(
    (response) =>
      response.url() === `${origin}/${fixture.movedSlug}?/restore` &&
      response.request().method() === "POST"
  )
  guard.allow(fixture.movedSlug, "restore")
  await form.locator('[type="submit"]').click()
  const actionResponse = await responsePromise
  const postRequest = actionResponse.request()
  const body = postRequest.postDataBuffer()
  if (!body) assert.fail("Restore POST body required")
  const contentType = postRequest.headers()["content-type"]
  const submitted = contentType?.startsWith("application/json")
    ? JSON.parse(body.toString())
    : parse(
        (
          await new Request(postRequest.url(), {
            method: "POST",
            headers: postRequest.headers(),
            body: new Uint8Array(body)
          }).formData()
        )
          .getAll("__superform_json")
          .join("")
      )
  const sentPageId =
    typeof submitted === "object" && submitted !== null && "pageId" in submitted
      ? submitted.pageId
      : null
  const result = await actionResponse.json()
  const evidence = JSON.stringify({ sentPageId, status: actionResponse.status(), result })
  assert.equal(sentPageId, fixture.pageId, `Restore sent wrong pageId: ${evidence}`)
  assert.equal(actionResponse.status(), 200, `Restore HTTP failure: ${evidence}`)
  assert.equal(result.type, "success", `Restore action failed: ${evidence}`)
  try {
    await expect(form).toHaveCount(0)
  } catch (error) {
    throw new Error(`Restore form remained: ${evidence}`, { cause: error })
  }
  guard.assertConsumed()
}

/**
 * Best-effort recovery is not UI proof. Every recovery write still targets
 * the manifest ID after a fresh ownership/hash read (or deleted-list ID
 * check).
 *
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {Fixture} fixture
 * @param {string} sourceHash
 * @param {string | null} originalLayout
 */
async function recoverFixture(request, token, fixture, sourceHash, originalLayout) {
  let moved = await readPage(request, token, fixture.movedSlug)
  if (moved.type === "missing") {
    /** @type {PageDeletedGet[]} */
    const deleted = await rpc(request, token, fixture.movedSlug, "page_get_deleted", {
      site_id: siteId,
      slug: fixture.movedSlug
    })
    if (
      deleted.some(
        (item) => item.page_id === fixture.pageId && item.slug === fixture.movedSlug
      )
    ) {
      /** @type {SessionModel} */
      const session = await rpc(request, token, fixture.movedSlug, "session_get", [token])
      assert.ok(Number.isSafeInteger(session?.user_id), "recovery actor required")
      await rpc(request, token, fixture.movedSlug, "page_restore", {
        site_id: siteId,
        page_id: fixture.pageId,
        user_id: session.user_id,
        ip_address: "127.0.0.1",
        revision_comments: "Restore disposable browser test fixture"
      })
      moved = await readPage(request, token, fixture.movedSlug)
    }
  }
  const source = await readPage(request, token, fixture.sourceSlug)
  if (moved.type === "found") {
    assert.equal(source.type, "missing", "source slug must remain vacant for recovery")
    const current = await assertOwnedPage(
      request,
      token,
      fixture,
      fixture.movedSlug,
      sourceHash
    )
    /** @type {SessionModel} */
    const session = await rpc(request, token, fixture.movedSlug, "session_get", [token])
    assert.ok(Number.isSafeInteger(session?.user_id), "recovery actor required")
    const actor = { site_id: siteId, user_id: session.user_id, ip_address: "127.0.0.1" }
    if (current.page.layout !== originalLayout) {
      await rpc(request, token, fixture.movedSlug, "page_set_layout", {
        ...actor,
        page_id: fixture.pageId,
        layout: originalLayout
      })
    }
    await assertOwnedPage(request, token, fixture, fixture.movedSlug, sourceHash)
    await rpc(request, token, fixture.movedSlug, "page_move", {
      ...actor,
      page: fixture.pageId,
      last_revision_id: current.page_revision.revision_id,
      new_slug: fixture.sourceSlug,
      revision_comments: "Restore disposable browser test slug"
    })
  }
  const final = await assertOwnedPage(
    request,
    token,
    fixture,
    fixture.sourceSlug,
    sourceHash
  )
  if (final.page.layout !== originalLayout) {
    /** @type {SessionModel} */
    const session = await rpc(request, token, fixture.sourceSlug, "session_get", [token])
    assert.ok(Number.isSafeInteger(session?.user_id), "recovery actor required")
    await rpc(request, token, fixture.sourceSlug, "page_set_layout", {
      site_id: siteId,
      page_id: fixture.pageId,
      user_id: session.user_id,
      ip_address: "127.0.0.1",
      layout: originalLayout
    })
  }
  assert.equal(
    (await assertOwnedPage(request, token, fixture, fixture.sourceSlug, sourceHash)).page
      .layout,
    originalLayout
  )
  await assertMissing(request, token, fixture.movedSlug)
}

/**
 * @param {string} stage @param {Fixture} fixture @param {string}
 *   sourceHash @param {unknown} error
 */
async function writePrivateFailure(stage, fixture, sourceHash, error) {
  const path = process.env.COBALT_PAGE_ACTION_ERROR_FILE
  assert.ok(path, "private failure artifact path required")
  const handle = await open(path, "w", 0o600)
  try {
    await handle.chmod(0o600)
    const detail = JSON.stringify({
      stage,
      pageId: fixture.pageId,
      sourceHash,
      error: error instanceof Error ? error.message : String(error)
    })
    await handle.writeFile(`${detail}\n`)
  } finally {
    await handle.close()
  }
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {Fixture} fixture
 * @param {WriteGuard} guard
 */
async function exercise(page, request, token, fixture, guard) {
  const initial = await readPage(request, token, fixture.sourceSlug)
  if (initial.type !== "found") assert.fail("newly created fixture page must exist")
  assert.equal(initial.data.page.page_id, fixture.pageId)
  assert.equal(initial.data.page.slug, fixture.sourceSlug)
  assert.ok(
    initial.data.page_revision.revision_id >= fixture.created[0].revisionId,
    "source revision must not predate creation"
  )
  const sourceHash = hash(initial.data.wikitext)
  const originalLayout = initial.data.page.layout
  assert.ok([null, "wikidot", "wikijump"].includes(originalLayout))
  await assertMissing(request, token, fixture.movedSlug)
  const destination = await readPage(request, token, fixture.destinationSlug)
  if (destination.type !== "found") assert.fail("protected destination page must exist")
  assert.equal(destination.data.page.page_id, fixture.destinationPageId)
  assert.equal(destination.data.page.slug, fixture.destinationSlug)
  assert.ok(
    destination.data.page_revision.revision_id >= fixture.created[1].revisionId,
    "destination revision must not predate creation"
  )
  const destinationHash = hash(destination.data.wikitext)
  const destinationRevision = destination.data.page_revision.revision_id
  let stage = "move"
  try {
    await assertOwnedPage(request, token, fixture, fixture.sourceSlug, sourceHash)
    await moveInBrowser(page, guard, fixture.sourceSlug, fixture.movedSlug)
    await assertMissing(request, token, fixture.sourceSlug)
    await assertOwnedPage(request, token, fixture, fixture.movedSlug, sourceHash)

    stage = "layout override"
    const override = originalLayout === "wikijump" ? "wikidot" : "wikijump"
    await assertOwnedPage(request, token, fixture, fixture.movedSlug, sourceHash)
    await selectLayout(page, guard, fixture.movedSlug, override)
    assert.equal(
      (await assertOwnedPage(request, token, fixture, fixture.movedSlug, sourceHash)).page
        .layout,
      override
    )

    stage = "layout default"
    await assertOwnedPage(request, token, fixture, fixture.movedSlug, sourceHash)
    await selectLayout(page, guard, fixture.movedSlug, null)
    assert.equal(
      (await assertOwnedPage(request, token, fixture, fixture.movedSlug, sourceHash)).page
        .layout,
      null
    )

    stage = "delete"
    await assertOwnedPage(request, token, fixture, fixture.movedSlug, sourceHash)
    await deleteInBrowser(page, guard, fixture.movedSlug)
    await assertDeleted(request, token, fixture)

    stage = "restore"
    await restoreInBrowser(page, guard, request, token, fixture)
    await assertOwnedPage(request, token, fixture, fixture.movedSlug, sourceHash)

    stage = "restore original layout"
    await assertOwnedPage(request, token, fixture, fixture.movedSlug, sourceHash)
    await selectLayout(page, guard, fixture.movedSlug, originalLayout)
    assert.equal(
      (await assertOwnedPage(request, token, fixture, fixture.movedSlug, sourceHash)).page
        .layout,
      originalLayout
    )

    stage = "restore original slug"
    await assertOwnedPage(request, token, fixture, fixture.movedSlug, sourceHash)
    await assertMissing(request, token, fixture.sourceSlug)
    await moveInBrowser(page, guard, fixture.movedSlug, fixture.sourceSlug)
    await assertMissing(request, token, fixture.movedSlug)
    const final = await assertOwnedPage(
      request,
      token,
      fixture,
      fixture.sourceSlug,
      sourceHash
    )
    assert.equal(final.page.layout, originalLayout, "fixture layout must match baseline")
    const untouched = await assertOwnedPage(
      request,
      token,
      {
        ...fixture,
        pageId: fixture.destinationPageId
      },
      fixture.destinationSlug,
      destinationHash
    )
    assert.equal(
      untouched.page_revision.revision_id,
      destinationRevision,
      "destination revision unchanged"
    )
    assert.deepEqual(guard.writes, [
      `/${fixture.sourceSlug}?/move`,
      `/${fixture.movedSlug}?/layout`,
      `/${fixture.movedSlug}?/layout`,
      `/${fixture.movedSlug}?/delete`,
      `/${fixture.movedSlug}?/restore`,
      `/${fixture.movedSlug}?/layout`,
      `/${fixture.movedSlug}?/move`
    ])
    guard.assertConsumed()
  } catch (error) {
    let failure = error
    try {
      await recoverFixture(request, token, fixture, sourceHash, originalLayout)
      const untouched = await assertOwnedPage(
        request,
        token,
        {
          ...fixture,
          pageId: fixture.destinationPageId
        },
        fixture.destinationSlug,
        destinationHash
      )
      assert.equal(untouched.page_revision.revision_id, destinationRevision)
    } catch (recoveryError) {
      failure = new AggregateError(
        [error, recoveryError],
        `${stage}: fixture recovery failed; original: ${String(error)}; recovery: ${String(recoveryError)}`
      )
    }
    await writePrivateFailure(stage, fixture, sourceHash, failure)
    throw failure
  }
}

test(
  "disposable page actions mutate only manifest page and restore its baseline",
  { timeout: 120_000 },
  async () => {
    const fixturePath = process.env.COBALT_PAGE_ACTION_FIXTURE
    const adminPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
    const gatewayPath = process.env.COBALT_LOCAL_PASSWORD_FILE
    assert.ok(
      fixturePath &&
        adminPath &&
        gatewayPath &&
        process.env.COBALT_PAGE_ACTION_ERROR_FILE,
      "fixture, admin password, gateway password, and private error file paths required"
    )
    const fixture = await readFixture(fixturePath)
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
        const { page, token } = await login(
          context,
          fixture,
          (await readFile(adminPath, "utf8")).trim()
        )
        try {
          await exercise(page, context.request, token, fixture, guard)
        } finally {
          console.log(JSON.stringify({ blockedExternalReads: guard.externalReads }))
        }
      } finally {
        await context.close()
      }
    } finally {
      await browser.close()
    }
  }
)
