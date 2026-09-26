import assert from "node:assert/strict"
import { randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"
import {
  origin,
  siteId,
  rpc,
  assertLivePage,
  listFiles,
  readFileBytes,
  requestLabel,
  guardBrowserWrites,
  assertActionSuccess
} from "./file-action-transport.mjs"

const fixturePath =
  "/home/osso/.local/share/cobalt-wiki/local-full/page-action-fixture.json"

/** @typedef {import("../../src/lib/server/deepwell/pageFile").PageFile} PageFile */
/** @typedef {import("../../src/lib/types").FileRevisionModel} FileRevisionModel */
/** @typedef {Awaited<ReturnType<typeof guardBrowserWrites>>} WriteGuard */
/**
 * @typedef {{
 *   sacrificial: true
 *   siteId: number
 *   siteSlug: string
 *   databaseLabel: string
 *   username: string
 *   sourceSlug: string
 *   destinationSlug: string
 *   pageId: number
 *   destinationPageId: number
 *   created: { slug: string; pageId: number }[]
 * }} Fixture
 */

/** @param {string} path */
async function readFixture(path) {
  /** @type {Fixture} */
  const fixture = JSON.parse(await readFile(path, "utf8"))
  assert.equal(fixture.sacrificial, true)
  assert.equal(fixture.siteId, siteId)
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.username, "cobalt-import")
  const match = /^local-action-proof:source-([a-f0-9]{16})$/.exec(fixture.sourceSlug)
  if (!match) assert.fail("unique disposable source required")
  assert.equal(fixture.destinationSlug, `local-action-proof:destination-${match[1]}`)
  assert.equal(fixture.pageId, 3000006134)
  assert.equal(fixture.destinationPageId, 3000006135)
  assert.deepEqual(
    fixture.created.map(({ slug, pageId }) => ({ slug, pageId })),
    [
      { slug: fixture.sourceSlug, pageId: fixture.pageId },
      { slug: fixture.destinationSlug, pageId: fixture.destinationPageId }
    ]
  )
  return fixture
}

/**
 * @param {import("@playwright/test").BrowserContext} context @param
 *   {string} username @param {string} password
 */
async function login(context, username, password) {
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  await page.locator('#login [name="nameOrEmail"]').fill(username)
  await page.locator('#login [name="password"]').fill(password)
  await page.locator('#login button[type="submit"]').click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (item) => item.name === "wikijump_token" && item.domain === "127.0.0.1"
  )
  if (!cookie) assert.fail("browser login required")
  return { page, token: decodeURIComponent(cookie.value) }
}

/** @param {import("@playwright/test").Page} page @param {string} slug */
async function openFiles(page, slug) {
  const response = await page.goto(`${origin}/${slug}`, { waitUntil: "networkidle" })
  assert.equal(response?.status(), 200)
  const wikijumpFiles = page.locator(".other-actions .button-files")
  if (await wikijumpFiles.isVisible()) {
    await wikijumpFiles.click()
  } else {
    await page.locator("#page-options-bottom #files-button").click()
  }
  const pane = page.locator(".file-panel")
  await expect(pane).toBeVisible()
  return pane
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {string} action @param
 *   {import("@playwright/test").Locator} control
 * @param {number | null} [fileId] @param {string | null} [name]
 */
async function clickMutation(
  page,
  guard,
  slug,
  action,
  control,
  fileId = null,
  name = null
) {
  const count = guard.writes.length
  const responsePromise = page.waitForResponse(
    (response) =>
      response.url() === `${origin}/${slug}?/${action}` &&
      response.request().method() === "POST"
  )
  guard.allow(slug, action, fileId, name)
  const [response] = await Promise.all([responsePromise, control.click()])
  await assertActionSuccess(response, action)
  assert.equal(guard.writes.length, count + 1, `${action} dispatch count`)
  guard.assertConsumed()
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug @param {number} pageId @param
 *   {number} fileId @param {string} name @param {Buffer} bytes @param
 *   {number} priorRevision
 */
async function assertFile(
  request,
  token,
  slug,
  pageId,
  fileId,
  name,
  bytes,
  priorRevision = 0
) {
  const { file, bytes: actual } = await readFileBytes(
    request,
    token,
    slug,
    pageId,
    fileId
  )
  assert.equal(file.name, name)
  assert.equal(file.revision_type === "delete", false)
  assert.ok(file.revision_id > priorRevision, "file revision must advance")
  assert.deepEqual(actual, bytes)
  const active = await listFiles(request, token, slug, pageId, false)
  assert.equal(active.filter((entry) => entry.file_id === fileId).length, 1)
  return file
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {Fixture} fixture @param {number} fileId @param
 *   {Map<string, PageFile[]>} baseline
 */
async function assertOriginalFiles(request, token, fixture, fileId, baseline) {
  /** @type {[number, string][]} */
  const pages = [
    [fixture.pageId, fixture.sourceSlug],
    [fixture.destinationPageId, fixture.destinationSlug]
  ]
  for (const [pageId, slug] of pages) {
    for (const deleted of [false, true]) {
      const key = `${pageId}:${deleted}`
      const current = await listFiles(request, token, slug, pageId, deleted)
      assert.deepEqual(
        current
          .filter((file) => file.file_id !== fileId)
          .sort((a, b) => a.file_id - b.file_id),
        baseline.get(key),
        `original ${key} file list changed`
      )
    }
  }
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {Fixture} fixture @param {number} fileId
 * @param {string[]} names
 */
async function recoverFile(request, token, fixture, fileId, names) {
  /** @type {[number, string][]} */
  const pages = [
    [fixture.pageId, fixture.sourceSlug],
    [fixture.destinationPageId, fixture.destinationSlug]
  ]
  /** @type {PageFile[]} */
  const found = []
  for (const [pageId, slug] of pages) {
    for (const deleted of [false, true]) {
      const files = await listFiles(request, token, slug, pageId, deleted)
      found.push(...files.filter((file) => file.file_id === fileId))
    }
  }
  assert.ok(
    found.length > 0 && found.length <= 2,
    "created file must be located uniquely"
  )
  assert.ok(
    found.every((file) => names.includes(file.name)),
    "recovery must own generated names"
  )
  const current = found.find((file) => file.revision_type !== "delete")
  if (!current) return // already tombstoned; never touch another file
  const source = current.page_id === fixture.pageId
  const destination = current.page_id === fixture.destinationPageId
  assert.ok(source || destination, "file moved outside protected pages")
  const slug = source ? fixture.sourceSlug : fixture.destinationSlug
  const session = await rpc(request, token, slug, "session_get", [token])
  assert.ok(Number.isSafeInteger(session?.user_id), "recovery actor required")
  const actor = { site_id: siteId, user_id: session.user_id }
  await rpc(request, token, slug, "file_delete", {
    ...actor,
    page_id: current.page_id,
    file: fileId,
    last_revision_id: current.revision_id,
    revision_comments: "Tombstone disposable file after browser test"
  })
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {number} fileId @param {string} action
 */
async function openFileAction(page, guard, slug, fileId, action) {
  const pane = await openFiles(page, slug)
  const row = pane.locator(`.file-row[data-id="${fileId}"]`)
  await expect(row).toBeVisible()
  const selector = action === "history" ? ".file-history" : `.${action}-file`
  const button = row.locator(selector)
  if (await button.count()) {
    await button.click()
  } else {
    await row
      .locator(".action a", {
        hasText: action === "edit" ? "Edit" : action === "move" ? "Move" : "History"
      })
      .click()
  }
  return pane
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {number} fileId @param {string} name
 * @param {Buffer | null} bytes
 */
async function editInBrowser(page, guard, slug, fileId, name, bytes) {
  const pane = await openFileAction(page, guard, slug, fileId, "edit")
  const form = pane.locator("#file-edit")
  await expect(form).toBeVisible()
  await form.locator('[name="name"]').fill(name)
  if (bytes) {
    await form.locator('[name="file"]').setInputFiles({
      name,
      mimeType: "text/plain",
      buffer: bytes
    })
  }
  await clickMutation(
    page,
    guard,
    slug,
    "fileEdit",
    form.locator('[type="submit"]'),
    fileId
  )
  await expect(form).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {number} fileId @param {string} destination
 */
async function moveInBrowser(page, guard, slug, fileId, destination) {
  const pane = await openFileAction(page, guard, slug, fileId, "move")
  const form = pane.locator("#file-move")
  await form.locator('[name="destinationPage"]').fill(destination)
  await clickMutation(
    page,
    guard,
    slug,
    "fileMove",
    form.locator('[type="submit"]'),
    fileId
  )
  await expect(form).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {number} fileId
 */
async function deleteInBrowser(page, guard, slug, fileId) {
  const pane = await openFiles(page, slug)
  const row = pane.locator(`.file-row[data-id="${fileId}"]`)
  await expect(row).toBeVisible()
  const button = row.locator(".delete-file, .action a:has-text('Delete')")
  await clickMutation(page, guard, slug, "fileDelete", button, fileId)
  await expect(row).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {number} fileId
 */
async function restoreInBrowser(page, guard, slug, fileId) {
  const pane = await openFiles(page, slug)
  await pane.locator(".deleted-file, .buttons input[value='Restore']").click()
  const row = pane.locator(`.file-row[data-id="${fileId}"]`)
  const restore = row.locator(".restore-file, .action a:has-text('Restore')")
  await expect(restore).toBeVisible()
  await restore.click()
  const form = pane.locator("#file-restore")
  await clickMutation(
    page,
    guard,
    slug,
    "fileRestore",
    form.locator('[type="submit"]'),
    fileId
  )
  await expect(form).toHaveCount(0)
}

/**
 * @param {import("@playwright/test").Page} page @param {WriteGuard} guard
 * @param {string} slug @param {number} fileId @param {number}
 *   revisionNumber
 */
async function rollbackInBrowser(page, guard, slug, fileId, revisionNumber) {
  const pane = await openFileAction(page, guard, slug, fileId, "history")
  await expect(pane.locator(".revision-list")).toBeVisible()
  const row = pane.locator(".revision-row").filter({
    has: page.locator(`.revision-number:text-is("${revisionNumber}")`)
  })
  await expect(row).toHaveCount(1)
  const rollback = row.locator(".revision-rollback, .action a")
  await clickMutation(page, guard, slug, "fileRollback", rollback, fileId)
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {Fixture} fixture
 */
async function captureFileBaseline(request, token, fixture) {
  /** @type {Map<string, PageFile[]>} */
  const baseline = new Map()
  /** @type {[number, string][]} */
  const pages = [
    [fixture.pageId, fixture.sourceSlug],
    [fixture.destinationPageId, fixture.destinationSlug]
  ]
  for (const [pageId, slug] of pages) {
    for (const deleted of [false, true]) {
      const files = await listFiles(request, token, slug, pageId, deleted)
      baseline.set(
        `${pageId}:${deleted}`,
        files.sort((a, b) => a.file_id - b.file_id)
      )
    }
  }
  return baseline
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token
 * @param {Fixture} fixture @param {Map<string, PageFile[]>} baseline
 * @param {string} name
 */
async function findCreatedFile(request, token, fixture, baseline, name) {
  const files = await listFiles(request, token, fixture.sourceSlug, fixture.pageId, false)
  const originalFiles = baseline.get(`${fixture.pageId}:false`)
  if (!originalFiles) assert.fail("source file baseline required")
  const originalIds = new Set(originalFiles.map((file) => file.file_id))
  const created = files.filter(
    (file) => file.name === name && !originalIds.has(file.file_id)
  )
  assert.ok(created.length <= 1, "ambiguous UI-created file")
  return created[0]?.file_id ?? null
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token @param {string} slug @param {number} pageId
 * @param {WriteGuard} guard @param {PageFile} current
 * @param {string} name @param {Buffer} bytes @param {Buffer | null} upload
 */
async function editAndAssertFile(
  page,
  request,
  token,
  slug,
  pageId,
  guard,
  current,
  name,
  bytes,
  upload
) {
  await editInBrowser(page, guard, slug, current.file_id, name, upload)
  return assertFile(
    request,
    token,
    slug,
    pageId,
    current.file_id,
    name,
    bytes,
    current.revision_id
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token @param {Fixture} fixture @param {WriteGuard} guard
 * @param {PageFile} current @param {number} firstId @param {number}
 *   createRevision
 * @param {string} name @param {Buffer} original
 */
async function rollbackAndAssertFile(
  page,
  request,
  token,
  fixture,
  guard,
  current,
  firstId,
  createRevision,
  name,
  original
) {
  /** @type {FileRevisionModel[]} */
  const history = await rpc(request, token, fixture.sourceSlug, "file_revision_range", {
    site_id: siteId,
    page_id: fixture.pageId,
    file_id: current.file_id,
    revision_number: current.revision_number + 1,
    revision_direction: "before",
    limit: 20
  })
  assert.ok(
    history.some(
      (revision) =>
        revision.revision_id === firstId && revision.revision_number === createRevision
    )
  )
  await rollbackInBrowser(
    page,
    guard,
    fixture.sourceSlug,
    current.file_id,
    createRevision
  )
  return assertFile(
    request,
    token,
    fixture.sourceSlug,
    fixture.pageId,
    current.file_id,
    name,
    original,
    current.revision_id
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token @param {Fixture} fixture @param {WriteGuard} guard
 * @param {PageFile} current @param {string} name @param {Buffer} original
 */
async function moveAndAssertFile(
  page,
  request,
  token,
  fixture,
  guard,
  current,
  name,
  original
) {
  await moveInBrowser(
    page,
    guard,
    fixture.sourceSlug,
    current.file_id,
    fixture.destinationSlug
  )
  const sourceFiles = await listFiles(
    request,
    token,
    fixture.sourceSlug,
    fixture.pageId,
    false
  )
  assert.ok(!sourceFiles.some((file) => file.file_id === current.file_id))
  return assertFile(
    request,
    token,
    fixture.destinationSlug,
    fixture.destinationPageId,
    current.file_id,
    name,
    original,
    current.revision_id
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token @param {Fixture} fixture @param {WriteGuard} guard
 * @param {PageFile} current
 */
async function deleteAndAssertFile(page, request, token, fixture, guard, current) {
  await deleteInBrowser(page, guard, fixture.destinationSlug, current.file_id)
  const deleted = await listFiles(
    request,
    token,
    fixture.destinationSlug,
    fixture.destinationPageId,
    true
  )
  const tombstones = deleted.filter(
    (file) => file.file_id === current.file_id && file.revision_type === "delete"
  )
  assert.equal(tombstones.length, 1)
  assert.ok(tombstones[0].revision_id > current.revision_id)
  assert.equal(tombstones[0].s3_hash, current.s3_hash)
  return tombstones[0]
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token @param {Fixture} fixture @param {WriteGuard} guard
 * @param {number} fileId @param {string} name @param {Buffer} original
 * @param {number} priorRevision
 */
async function restoreAndAssertFile(
  page,
  request,
  token,
  fixture,
  guard,
  fileId,
  name,
  original,
  priorRevision
) {
  await restoreInBrowser(page, guard, fixture.destinationSlug, fileId)
  await assertFile(
    request,
    token,
    fixture.destinationSlug,
    fixture.destinationPageId,
    fileId,
    name,
    original,
    priorRevision
  )
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token @param {Fixture} fixture @param {WriteGuard} guard
 * @param {number} fileId
 */
async function deleteAndAssertCleanup(page, request, token, fixture, guard, fileId) {
  await deleteInBrowser(page, guard, fixture.destinationSlug, fileId)
  const finalFiles = await listFiles(
    request,
    token,
    fixture.destinationSlug,
    fixture.destinationPageId,
    false
  )
  assert.ok(!finalFiles.some((file) => file.file_id === fileId))
  assertWriteSequence(guard, fixture)
}

/**
 * @param {import("@playwright/test").Page} page @param
 *   {import("@playwright/test").APIRequestContext} request @param {string}
 *   token @param {Fixture} fixture @param {WriteGuard} guard
 */
async function exercise(page, request, token, fixture, guard) {
  const source = await assertLivePage(request, token, fixture.sourceSlug, fixture.pageId)
  const destination = await assertLivePage(
    request,
    token,
    fixture.destinationSlug,
    fixture.destinationPageId
  )
  const pageRevisions = [
    source.page_revision.revision_id,
    destination.page_revision.revision_id
  ]
  const pageHashes = [source.wikitext, destination.wikitext]
  const baseline = await captureFileBaseline(request, token, fixture)
  const suffix = `${fixture.sourceSlug.split("-").at(-1)}-${randomBytes(8).toString("hex")}`
  const name = `ui-file-${suffix}.txt`
  const renamed = `ui-renamed-${suffix}.txt`
  const original = Buffer.from(`Cobalt UI file ${suffix}\n`)
  const replacement = Buffer.from(`Cobalt UI replacement ${suffix}\n`)
  assert.ok(
    ![...baseline.values()].flat().some((file) => [name, renamed].includes(file.name))
  )
  /** @type {number | null} */
  let fileId = null
  let stage = "upload"
  try {
    const createdFileId = await uploadFixtureFile(
      page,
      request,
      token,
      fixture,
      guard,
      baseline,
      name,
      original
    )
    fileId = createdFileId
    let current = await assertFile(
      request,
      token,
      fixture.sourceSlug,
      fixture.pageId,
      fileId,
      name,
      original
    )
    const createRevision = current.revision_number
    const firstId = current.revision_id

    stage = "rename"
    current = await editAndAssertFile(
      page,
      request,
      token,
      fixture.sourceSlug,
      fixture.pageId,
      guard,
      current,
      renamed,
      original,
      null
    )

    stage = "replace"
    current = await editAndAssertFile(
      page,
      request,
      token,
      fixture.sourceSlug,
      fixture.pageId,
      guard,
      current,
      renamed,
      replacement,
      replacement
    )

    stage = "history and rollback"
    current = await rollbackAndAssertFile(
      page,
      request,
      token,
      fixture,
      guard,
      current,
      firstId,
      createRevision,
      name,
      original
    )

    stage = "move"
    current = await moveAndAssertFile(
      page,
      request,
      token,
      fixture,
      guard,
      current,
      name,
      original
    )

    stage = "delete"
    const tombstone = await deleteAndAssertFile(
      page,
      request,
      token,
      fixture,
      guard,
      current
    )

    stage = "restore"
    await restoreAndAssertFile(
      page,
      request,
      token,
      fixture,
      guard,
      fileId,
      name,
      original,
      tombstone.revision_id
    )
    stage = "cleanup"
    await deleteAndAssertCleanup(page, request, token, fixture, guard, fileId)
  } catch (error) {
    fileId ??= await findCreatedFile(request, token, fixture, baseline, name)
    await recoverFailedFile(
      request,
      token,
      fixture,
      fileId,
      [name, renamed],
      stage,
      error
    )
  } finally {
    await assertOriginalFiles(request, token, fixture, fileId ?? -1, baseline)
    await assertOriginalPages(request, token, fixture, pageRevisions, pageHashes)
  }
}

/**
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {Fixture} fixture
 * @param {WriteGuard} guard
 * @param {Map<string, PageFile[]>} baseline
 * @param {string} name
 * @param {Buffer} original
 */
async function uploadFixtureFile(
  page,
  request,
  token,
  fixture,
  guard,
  baseline,
  name,
  original
) {
  const pane = await openFiles(page, fixture.sourceSlug)
  await pane.locator(".upload-file, .buttons input[value='Upload']").click()
  const form = pane.locator("#file-upload")
  await form.locator('[name="file"]').setInputFiles({
    name,
    mimeType: "text/plain",
    buffer: original
  })
  await form.locator('[name="name"]').fill(name)
  await clickMutation(
    page,
    guard,
    fixture.sourceSlug,
    "fileUpload",
    form.locator('[type="submit"]'),
    null,
    name
  )
  await expect(form).toHaveCount(0)
  const fileId = await findCreatedFile(request, token, fixture, baseline, name)
  assert.ok(fileId !== null, "unique UI-created file required")
  return fileId
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {Fixture} fixture
 * @param {number | null} fileId
 * @param {string[]} names
 * @param {string} stage
 * @param {unknown} error
 */
async function recoverFailedFile(request, token, fixture, fileId, names, stage, error) {
  if (fileId !== null) {
    try {
      await recoverFile(request, token, fixture, fileId, names)
    } catch (recoveryError) {
      throw new AggregateError(
        [error, recoveryError],
        `${stage}: recovery failed for disposable file ${fileId}`,
        { cause: recoveryError }
      )
    }
  }
  throw new Error(`${stage}: ${String(error)}`, { cause: error })
}

/**
 * @param {import("@playwright/test").APIRequestContext} request
 * @param {string} token
 * @param {Fixture} fixture
 * @param {number[]} revisions
 * @param {string[]} hashes
 */
async function assertOriginalPages(request, token, fixture, revisions, hashes) {
  /** @type {[number, string][]} */
  const pages = [
    [fixture.pageId, fixture.sourceSlug],
    [fixture.destinationPageId, fixture.destinationSlug]
  ]
  for (const [index, [pageId, slug]] of pages.entries()) {
    const view = await assertLivePage(request, token, slug, pageId)
    assert.equal(view.page_revision.revision_id, revisions[index])
    assert.equal(view.wikitext, hashes[index])
  }
}

/** @param {WriteGuard} guard @param {Fixture} fixture */
function assertWriteSequence(guard, fixture) {
  guard.assertConsumed()
  assert.deepEqual(guard.writes, [
    `/${fixture.sourceSlug}?/fileUpload`,
    `/${fixture.sourceSlug}?/fileEdit`,
    `/${fixture.sourceSlug}?/fileEdit`,
    `/${fixture.sourceSlug}?/fileRollback`,
    `/${fixture.sourceSlug}?/fileMove`,
    `/${fixture.destinationSlug}?/fileDelete`,
    `/${fixture.destinationSlug}?/fileRestore`,
    `/${fixture.destinationSlug}?/fileDelete`
  ])
}

test(
  "disposable file UI lifecycle preserves original files and pages",
  { timeout: 120_000 },
  async () => {
    const adminPath = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
    const gatewayPath = process.env.COBALT_LOCAL_PASSWORD_FILE
    assert.ok(adminPath && gatewayPath, "admin and gateway password file paths required")
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
        const guard = await guardBrowserWrites(context, [
          fixture.sourceSlug,
          fixture.destinationSlug
        ])
        const { page, token } = await login(
          context,
          fixture.username,
          (await readFile(adminPath, "utf8")).trim()
        )
        /** @type {string[]} */
        const failedRequests = []
        /** @type {string[]} */
        const pageErrors = []
        page.on("requestfailed", (request) => {
          failedRequests.push(
            requestLabel(request, [fixture.sourceSlug, fixture.destinationSlug])
          )
        })
        page.on("pageerror", (error) =>
          pageErrors.push(error.name === "Error" ? "Error" : "other")
        )
        try {
          await exercise(page, context.request, token, fixture, guard)
        } finally {
          console.info(
            `File action diagnostics: ${JSON.stringify({
              writes: guard.writes,
              blocked: guard.blocked,
              decoded: guard.decoded,
              failedRequests,
              pageErrors,
              externalReads: guard.externalReads
            })}`
          )
        }
      } finally {
        await context.close()
      }
    } finally {
      await browser.close()
    }
  }
)
