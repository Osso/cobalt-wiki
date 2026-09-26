import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const siteId = 6000000
const fixturePath =
  "/home/osso/.local/share/cobalt-wiki/local-full/page-action-fixture.json"

/** @typedef {import("../../src/lib/server/deepwell/pageFile").PageFile} PageFile */
/** @typedef {import("../../src/lib/server/deepwell/views").PageView} PageView */
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
  assert.ok(match, "unique disposable source required")
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
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug @param {string} method @param
 *   {unknown} params
 */
async function rpc(request, token, slug, method, params) {
  const response = await request.post(backend, {
    headers: {
      "X-Deepwell-Site-Id": String(siteId),
      "X-Deepwell-Page": slug,
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
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug
 */
async function readPage(request, token, slug) {
  /** @type {PageView} */
  return rpc(request, token, slug, "page_view", {
    site_id: siteId,
    locales: ["en"],
    session_token: token,
    route: { slug, extra: "" }
  })
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug @param {number} pageId
 */
async function assertLivePage(request, token, slug, pageId) {
  const view = await readPage(request, token, slug)
  assert.equal(view.type, "found", `${slug} must remain live`)
  if (view.type !== "found") assert.fail("fixture page missing")
  assert.equal(view.data.page.page_id, pageId)
  assert.equal(view.data.page.slug, slug)
  return view.data
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug @param {number} pageId @param
 *   {boolean} deleted
 */
async function listFiles(request, token, slug, pageId, deleted) {
  /** @type {PageFile[]} */
  return rpc(request, token, slug, "page_get_files", {
    site_id: siteId,
    page_id: pageId,
    deleted
  })
}

/**
 * @param {import("@playwright/test").APIRequestContext} request @param
 *   {string} token @param {string} slug @param {number} pageId @param
 *   {number} fileId
 */
async function readFileBytes(request, token, slug, pageId, fileId) {
  /** @type {PageFile} */
  const file = await rpc(request, token, slug, "file_get", {
    site_id: siteId,
    page_id: pageId,
    file: fileId,
    data: true
  })
  assert.equal(file.file_id, fileId)
  assert.equal(file.page_id, pageId)
  assert.ok(
    typeof file.data === "string" && /^[a-f0-9]*$/.test(file.data),
    "file_get must return hex bytes"
  )
  const bytes = Buffer.from(file.data, "hex")
  assert.equal(file.size, bytes.length)
  assert.equal(file.s3_hash, createHash("sha512").update(bytes).digest("hex"))
  return { file, bytes }
}

/**
 * @param {import("@playwright/test").BrowserContext} context @param
 *   {string[]} slugs
 */
async function guardBrowserWrites(context, slugs) {
  /**
   * @type {{
   *   slug: string
   *   action: string
   *   fileId: number | null
   * } | null}
   */
  let permitted = null
  /** @type {string[]} */
  const writes = []
  /** @type {string[]} */
  const blocked = []
  /** @type {string[]} */
  const externalReads = []
  await context.route("**/*", (route) => {
    const request = route.request()
    const url = new URL(request.url())
    if (url.origin !== origin) {
      if (["GET", "HEAD", "OPTIONS"].includes(request.method())) {
        externalReads.push(`${request.method()} ${url.origin}`)
      } else {
        blocked.push(`${request.method()} foreign origin`)
      }
      return route.abort()
    }
    if (["GET", "HEAD", "OPTIONS"].includes(request.method())) return route.continue()
    const path = `${url.pathname}${url.search}`
    const login = path === "/-/login" || path === "/-/login?/login"
    const read = slugs.some((slug) =>
      ["fileList", "fileHistory"].some((action) => path === `/${slug}?/${action}`)
    )
    const mutation = permitted && path === `/${permitted.slug}?/${permitted.action}`
    if (request.method() === "POST" && (login || read || mutation)) {
      if (mutation) {
        const body = request.postData() ?? ""
        const pageId = slugs.indexOf(permitted.slug) === 0 ? 3000006134 : 3000006135
        const correctFile =
          permitted.fileId === null
            ? body.includes(`ui-file-${permitted.slug.split("-").at(-1)}.txt`)
            : body.includes(`"fileId":${permitted.fileId}`)
        const correctPage = body.includes(`"pageId":${pageId}`)
        const correctSite = body.includes(`"siteId":${siteId}`)
        if (!correctFile || !correctPage || !correctSite) {
          blocked.push(`POST ${path} wrong fixture identity`)
          return route.abort()
        }
        writes.push(path)
        permitted = null
      }
      return route.continue()
    }
    blocked.push(`${request.method()} ${path}`)
    return route.abort()
  })
  return {
    writes,
    externalReads,
    /**
     * @param {string} slug @param {string} action @param {number | null}
     *   fileId
     */
    allow(slug, action, fileId) {
      assert.ok(slugs.includes(slug))
      assert.equal(permitted, null, "previous write not consumed")
      permitted = { slug, action, fileId }
    },
    assertConsumed() {
      assert.equal(permitted, null, "UI did not send expected mutation")
      assert.deepEqual(blocked, [], "unexpected browser write blocked")
    }
  }
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
  assert.ok(cookie, "browser login required")
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
    const options = page.locator("#page-options-bottom-2")
    if (!(await options.isVisible())) await page.locator("#more-options-button").click()
    await options.locator("#files-button").click()
  }
  const pane = page.locator(".file-panel")
  await expect(pane).toBeVisible()
  return pane
}

/**
 * @param {import("@playwright/test").Page} page @param {object} guard
 * @param {string} slug @param {string} action @param
 *   {import("@playwright/test").Locator} control
 */
async function clickMutation(page, guard, slug, action, control, fileId = null) {
  const count = guard.writes.length
  guard.allow(slug, action, fileId)
  await control.click()
  await expect.poll(() => guard.writes.length).toBe(count + 1)
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
  for (const [pageId, slug] of [
    [fixture.pageId, fixture.sourceSlug],
    [fixture.destinationPageId, fixture.destinationSlug]
  ]) {
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
 */
async function recoverFile(request, token, fixture, fileId) {
  const pages = [
    [fixture.pageId, fixture.sourceSlug],
    [fixture.destinationPageId, fixture.destinationSlug]
  ]
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
    found.every(
      (file) => file.name.startsWith("ui-file-") || file.name.startsWith("ui-renamed-")
    ),
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
 * @param {import("@playwright/test").Page} page @param {object} guard
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
 * @param {import("@playwright/test").Page} page @param {object} guard
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
 * @param {import("@playwright/test").Page} page @param {object} guard
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
 * @param {import("@playwright/test").Page} page @param {object} guard
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
 * @param {import("@playwright/test").Page} page @param {object} guard
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
 * @param {import("@playwright/test").Page} page @param {object} guard
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
 * @param {import("@playwright/test").Page} page @param
 *   {import("@playwright/test").APIRequestContext} request @param {string}
 *   token @param {object} fixture @param {object} guard
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
  const baseline = new Map()
  for (const [pageId, slug] of [
    [fixture.pageId, fixture.sourceSlug],
    [fixture.destinationPageId, fixture.destinationSlug]
  ]) {
    for (const deleted of [false, true]) {
      const files = await listFiles(request, token, slug, pageId, deleted)
      baseline.set(
        `${pageId}:${deleted}`,
        files.sort((a, b) => a.file_id - b.file_id)
      )
    }
  }
  const suffix = fixture.sourceSlug.split("-").at(-1)
  const name = `ui-file-${suffix}.txt`
  const renamed = `ui-renamed-${suffix}.txt`
  const original = Buffer.from(`Cobalt UI file ${suffix}\n`)
  const replacement = Buffer.from(`Cobalt UI replacement ${suffix}\n`)
  assert.ok(
    ![...baseline.values()].flat().some((file) => [name, renamed].includes(file.name))
  )
  let fileId = null
  let stage = "upload"
  try {
    const pane = await openFiles(page, fixture.sourceSlug)
    await pane.locator(".upload-file, .buttons input[value='Upload']").click()
    const form = pane.locator("#file-upload")
    await form
      .locator('[name="file"]')
      .setInputFiles({ name, mimeType: "text/plain", buffer: original })
    await form.locator('[name="name"]').fill(name)
    await clickMutation(
      page,
      guard,
      fixture.sourceSlug,
      "fileUpload",
      form.locator('[type="submit"]')
    )
    await expect(form).toHaveCount(0)
    const uploaded = (
      await listFiles(request, token, fixture.sourceSlug, fixture.pageId, false)
    ).filter(
      (file) =>
        file.name === name &&
        !baseline
          .get(`${fixture.pageId}:false`)
          .some((old) => old.file_id === file.file_id)
    )
    assert.equal(uploaded.length, 1, "unique UI-created file required")
    fileId = uploaded[0].file_id
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
    await editInBrowser(page, guard, fixture.sourceSlug, fileId, renamed, null)
    current = await assertFile(
      request,
      token,
      fixture.sourceSlug,
      fixture.pageId,
      fileId,
      renamed,
      original,
      current.revision_id
    )

    stage = "replace"
    await editInBrowser(page, guard, fixture.sourceSlug, fileId, renamed, replacement)
    current = await assertFile(
      request,
      token,
      fixture.sourceSlug,
      fixture.pageId,
      fileId,
      renamed,
      replacement,
      current.revision_id
    )

    stage = "history and rollback"
    /** @type {FileRevisionModel[]} */
    const history = await rpc(request, token, fixture.sourceSlug, "file_revision_range", {
      site_id: siteId,
      page_id: fixture.pageId,
      file_id: fileId,
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
    await rollbackInBrowser(page, guard, fixture.sourceSlug, fileId, createRevision)
    current = await assertFile(
      request,
      token,
      fixture.sourceSlug,
      fixture.pageId,
      fileId,
      name,
      original,
      current.revision_id
    )

    stage = "move"
    await moveInBrowser(page, guard, fixture.sourceSlug, fileId, fixture.destinationSlug)
    const sourceFiles = await listFiles(
      request,
      token,
      fixture.sourceSlug,
      fixture.pageId,
      false
    )
    assert.ok(!sourceFiles.some((file) => file.file_id === fileId))
    current = await assertFile(
      request,
      token,
      fixture.destinationSlug,
      fixture.destinationPageId,
      fileId,
      name,
      original,
      current.revision_id
    )

    stage = "delete"
    await deleteInBrowser(page, guard, fixture.destinationSlug, fileId)
    const deleted = await listFiles(
      request,
      token,
      fixture.destinationSlug,
      fixture.destinationPageId,
      true
    )
    const tombstones = deleted.filter(
      (file) => file.file_id === fileId && file.revision_type === "delete"
    )
    assert.equal(tombstones.length, 1)
    assert.ok(tombstones[0].revision_id > current.revision_id)
    assert.equal(tombstones[0].s3_hash, current.s3_hash)

    stage = "restore"
    await restoreInBrowser(page, guard, fixture.destinationSlug, fileId)
    current = await assertFile(
      request,
      token,
      fixture.destinationSlug,
      fixture.destinationPageId,
      fileId,
      name,
      original,
      tombstones[0].revision_id
    )
    stage = "cleanup"
    await deleteInBrowser(page, guard, fixture.destinationSlug, fileId)
    const finalFiles = await listFiles(
      request,
      token,
      fixture.destinationSlug,
      fixture.destinationPageId,
      false
    )
    assert.ok(!finalFiles.some((file) => file.file_id === fileId))
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
  } catch (error) {
    if (fileId === null) {
      const candidates = await listFiles(
        request,
        token,
        fixture.sourceSlug,
        fixture.pageId,
        false
      )
      const created = candidates.filter(
        (file) =>
          file.name === name &&
          !baseline
            .get(`${fixture.pageId}:false`)
            .some((old) => old.file_id === file.file_id)
      )
      if (created.length === 1) fileId = created[0].file_id
    }
    if (fileId !== null) {
      try {
        await recoverFile(request, token, fixture, fileId)
      } catch (recoveryError) {
        throw new AggregateError(
          [error, recoveryError],
          `${stage}: recovery failed for disposable file ${fileId}`
        )
      }
    }
    throw new Error(
      `${stage}: ${error instanceof Error ? error.message : String(error)}`,
      { cause: error }
    )
  } finally {
    await assertOriginalFiles(request, token, fixture, fileId ?? -1, baseline)
    for (const [index, [pageId, slug]] of [
      [fixture.pageId, fixture.sourceSlug],
      [fixture.destinationPageId, fixture.destinationSlug]
    ].entries()) {
      const view = await assertLivePage(request, token, slug, pageId)
      assert.equal(view.page_revision.revision_id, pageRevisions[index])
      assert.equal(view.wikitext, pageHashes[index])
    }
  }
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
        await exercise(page, context.request, token, fixture, guard)
      } finally {
        await context.close()
      }
    } finally {
      await browser.close()
    }
  }
)
