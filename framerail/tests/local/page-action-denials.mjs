import assert from "node:assert/strict"
import { createHash, randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { expect, chromium } from "@playwright/test"
import {
  rpc,
  origin,
  siteId,
  listFiles,
  readFileBytes,
  guardBrowserWrites
} from "./file-action-transport.mjs"
import { openFiles, clickMutation } from "./file-action-lifecycle.mjs"

const { stringify } = await import(
  new URL("../../node_modules/.pnpm/node_modules/devalue/index.js", import.meta.url).href
)
const fixturePath =
  "/home/osso/.local/share/cobalt-wiki/local-full/page-action-fixture.json"
const observerPath =
  "/home/osso/.local/share/cobalt-wiki/local-full/watching-proof/browser-fixture.json"
/** @typedef {import("../../src/lib/server/deepwell/views").PageView} PageView */
/** @typedef {Extract<PageView, { type: "found" }>["data"]} PageData */
/** @typedef {import("../../src/lib/server/deepwell/pageFile").PageFile} PageFile */
/** @typedef {Awaited<ReturnType<typeof login>>} Actor */
/** @typedef {ReturnType<typeof fingerprint>} Fingerprint */
/** @typedef {Awaited<ReturnType<typeof readFileLists>>} FileLists */
/** @typedef {Awaited<ReturnType<typeof readFixtures>>["fixture"]} Fixture */
/** @typedef {Awaited<ReturnType<typeof guardObserverWrites>>} ObserverGuard */
/** @typedef {Awaited<ReturnType<typeof guardBrowserWrites>>} AdminGuard */
/**
 * @typedef {{
 *   action: string
 *   body: Record<string, string | number | boolean | string[] | number[]>
 *   format?: "superform" | "json" | "form"
 * }} DenialCase
 */

const allowedDenials = new Set([
  "move",
  "delete",
  "parentSet",
  "layout",
  "setTags",
  "blockSet",
  "fileDelete",
  "fileEdit",
  "fileMove"
])

/** @typedef {{ action: string; payload: string }} ObserverAllowance */

/**
 * @param {import("@playwright/test").Request} request @param {URL} url
 *   @param {Fixture} fixture
 */
function isObserverRead(request, url, fixture) {
  const stylesheet =
    url.origin === "https://d3g0gp89917ko0.cloudfront.net" &&
    request.method() === "GET" &&
    request.resourceType() === "stylesheet"
  if (stylesheet) return true
  if (url.origin !== origin) return false
  if (["GET", "HEAD", "OPTIONS"].includes(request.method())) return true
  if (request.method() !== "POST") return false
  const path = `${url.pathname}${url.search}`
  if (["/-/login", "/-/login?/login"].includes(path)) return true
  return ["fileList", "fileHistory"].some(
    (action) => path === `/${fixture.sourceSlug}?/${action}`
  )
}

/**
 * @param {import("@playwright/test").Request} request @param {URL} url
 *   @param {Fixture} fixture @param {ObserverAllowance|null} permitted
 */
function matchesObserverWrite(request, url, fixture, permitted) {
  if (!permitted) return false
  const localPost = url.origin === origin && request.method() === "POST"
  const sameAction =
    `${url.pathname}${url.search}` === `/${fixture.sourceSlug}?/${permitted.action}`
  const samePayload = request.postDataBuffer()?.toString() === permitted.payload
  return localPost && sameAction && samePayload
}

/**
 * @param {import("@playwright/test").BrowserContext} context @param
 *   {Fixture} fixture
 */
async function guardObserverWrites(context, fixture) {
  /** @type {ObserverAllowance | null} */
  let permitted = null
  /** @type {number | null} */
  let disposableFileId = null
  /** @type {string[]} */
  const blocked = []
  await context.route("**/*", (route) => {
    const request = route.request()
    const url = new URL(request.url())
    if (isObserverRead(request, url, fixture)) return route.continue()
    const path = `${url.pathname}${url.search}`
    if (matchesObserverWrite(request, url, fixture, permitted)) {
      permitted = null
      return route.continue()
    }
    blocked.push(`${request.method()} ${url.origin}${path}`)
    return route.abort()
  })
  return {
    /** @param {number} fileId */
    setFileId(fileId) {
      assert.ok(Number.isSafeInteger(fileId) && fileId > 0)
      disposableFileId = fileId
    },
    /**
     * @param {string} action @param {Record<string, string | number |
     *   boolean | string[] | number[]>} body @param {string} payload
     */
    allow(action, body, payload) {
      assert.equal(permitted, null, "previous observer POST not consumed")
      assert.ok(allowedDenials.has(action), "unlisted observer action")
      if (action !== "setTags" && action !== "blockSet") {
        assert.equal(body.siteId, siteId, "observer action site identity")
      }
      if (action !== "setTags") {
        assert.equal(body.pageId, fixture.pageId, "observer action page identity")
      }
      if (action.startsWith("file")) {
        assert.ok(disposableFileId !== null, "owned file ID required")
        assert.equal(body.fileId, disposableFileId, "observer action file identity")
      }
      permitted = { action, payload }
    },
    assertConsumed() {
      assert.equal(permitted, null, "observer action POST not sent")
      assert.deepEqual(blocked, [], "unexpected observer browser request")
    }
  }
}

async function readPasswords() {
  const fixtureFile = process.env.COBALT_PAGE_ACTION_FIXTURE
  const adminFile = process.env.COBALT_LOCAL_ADMIN_PASSWORD_FILE
  const gatewayFile = process.env.COBALT_LOCAL_PASSWORD_FILE
  assert.ok(
    fixtureFile === fixturePath && adminFile && gatewayFile,
    "COBALT_PAGE_ACTION_FIXTURE must name disposable fixture; admin and gateway password files required"
  )
  return {
    adminPassword: (await readFile(adminFile, "utf8")).trim(),
    gatewayPassword: (await readFile(gatewayFile, "utf8")).trim()
  }
}

async function readFixtures() {
  /**
   * @type {{
   *   sacrificial: boolean
   *   siteId: number
   *   siteSlug: string
   *   databaseLabel: string
   *   username: string
   *   sourceSlug: string
   *   movedSlug: string
   *   destinationSlug: string
   *   pageId: number
   *   destinationPageId: number
   *   created: { slug: string; pageId: number; revisionId: number }[]
   * }}
   */
  const fixture = JSON.parse(await readFile(fixturePath, "utf8"))
  const observer = JSON.parse(await readFile(observerPath, "utf8"))
  assert.equal(fixture.sacrificial, true)
  assert.equal(fixture.siteId, siteId)
  assert.equal(fixture.siteSlug, "cobalt-company")
  assert.equal(fixture.databaseLabel, "cobalt_local_full")
  assert.equal(fixture.username, "cobalt-import")
  const match = /^local-action-proof:source-([a-f0-9]{16})$/.exec(fixture.sourceSlug)
  assert.ok(match, "disposable source slug required")
  assert.equal(fixture.movedSlug, `local-action-proof:moved-${match[1]}`)
  assert.equal(fixture.destinationSlug, `local-action-proof:destination-${match[1]}`)
  assert.equal(fixture.pageId, 3000006134)
  assert.equal(fixture.destinationPageId, 3000006135)
  assert.ok(Array.isArray(fixture.created), "created-page manifest required")
  assert.deepEqual(
    fixture.created.map(({ slug, pageId }) => ({ slug, pageId })),
    [
      { slug: fixture.sourceSlug, pageId: fixture.pageId },
      { slug: fixture.destinationSlug, pageId: fixture.destinationPageId }
    ]
  )
  assert.ok(
    fixture.created.every(
      ({ revisionId }) => Number.isSafeInteger(revisionId) && revisionId > 0
    )
  )
  assert.equal(observer.sacrificial, true)
  assert.equal(observer.email_must_remain_disabled, true)
  assert.equal(observer.site_id, siteId)
  assert.equal(observer.origin, origin)
  assert.ok(Number.isSafeInteger(observer.user_id))
  assert.ok(observer.user_id > 0)
  assert.ok(typeof observer.username === "string" && observer.username.length > 0)
  assert.ok(typeof observer.password === "string" && observer.password.length > 0)
  return { fixture, observer }
}

/**
 * @param {import("@playwright/test").BrowserContext} context @param
 *   {string} username @param {string} password
 */
async function login(context, username, password) {
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  await expect(page.locator("#login")).toBeVisible()
  await page.locator('#login [name="nameOrEmail"]').fill(username)
  await page.locator('#login [name="password"]').fill(password)
  await page.locator('#login button[type="submit"]').click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (entry) => entry.name === "wikijump_token" && entry.domain === "127.0.0.1"
  )
  assert.ok(cookie, "authenticated browser cookie required")
  return { page, context, token: decodeURIComponent(cookie.value) }
}

/**
 * @param {Actor} actor @param {string} slug @param {number} pageId
 * @returns {Promise<PageData>}
 */
async function readPage(actor, slug, pageId) {
  /** @type {PageView} */
  const view = await rpc(actor.context.request, actor.token, slug, "page_view", {
    site_id: siteId,
    locales: ["en"],
    session_token: actor.token,
    route: { slug, extra: "" }
  })
  assert.equal(view.type, "found", `${slug} must remain live`)
  if (view.type !== "found") assert.fail("fixture page missing")
  assert.equal(view.data.page.page_id, pageId, "only manifest ID allowed")
  assert.equal(view.data.page.slug, slug)
  return view.data
}

/**
 * @param {Actor} actor @param {string} slug @param {number} pageId
 * @returns {Promise<number[]>}
 */
async function readParents(actor, slug, pageId) {
  return rpc(actor.context.request, actor.token, slug, "parent_get_all", {
    site_id: siteId,
    page: pageId
  })
}

/** @param {PageData} page @param {number[]} parents */
function fingerprint(page, parents) {
  return {
    pageId: page.page.page_id,
    slug: page.page.slug,
    revision: page.page_revision.revision_id,
    sourceHash: createHash("sha256").update(page.wikitext).digest("hex"),
    layout: page.page.layout,
    tags: page.page_revision.tags,
    parents
  }
}

/** @param {Actor} actor @param {string} slug @param {number} pageId */
async function readFingerprint(actor, slug, pageId) {
  const page = await readPage(actor, slug, pageId)
  const parents = await readParents(actor, slug, pageId)
  return fingerprint(page, parents)
}

/**
 * @param {import("@playwright/test").Page} page @param {ObserverGuard}
 *   guard @param {string} slug @param {string} action @param
 *   {Record<string, string | number | boolean | string[] | number[]>}
 *   body
 * @param {"superform" | "json" | "form"} [format]
 */
async function postAction(page, guard, slug, action, body, format = "superform") {
  const payload =
    format === "json"
      ? JSON.stringify(body)
      : new URLSearchParams(
          format === "superform"
            ? { __superform_json: stringify(body) }
            : { changes: String(body.changes) }
        ).toString()
  guard.allow(action, body, payload)
  const result = await page.evaluate(
    async ({ slug, action, payload, format }) => {
      const headers = {
        accept: "application/json",
        "x-sveltekit-action": "true"
      }
      const response = await fetch(`/${slug}?/${action}`, {
        method: "POST",
        headers,
        body: format === "json" ? payload : new URLSearchParams(payload)
      })
      const result = await response.json()
      return { status: response.status, type: result.type, actionStatus: result.status }
    },
    { slug, action, payload, format }
  )
  guard.assertConsumed()
  return result
}

/**
 * @param {Actor} admin @param {Fixture} fixture @param {Fingerprint}
 *   baseline @param {boolean} originalBlocked @param {string} action
 * @param {PageView} current
 */
async function recoverPage(admin, fixture, baseline, originalBlocked, action, current) {
  const slug = fixture.sourceSlug
  const actor = {
    site_id: siteId,
    user_id: (
      await rpc(admin.context.request, admin.token, slug, "session_get", [admin.token])
    ).user_id,
    ip_address: "127.0.0.1"
  }
  assert.ok(Number.isSafeInteger(actor.user_id), "admin recovery actor required")
  if (action === "move" && current.type === "missing") {
    const moved = await readPage(admin, fixture.movedSlug, fixture.pageId)
    const movedState = fingerprint(
      moved,
      await readParents(admin, fixture.movedSlug, fixture.pageId)
    )
    assert.equal(movedState.sourceHash, baseline.sourceHash)
    await rpc(admin.context.request, admin.token, fixture.movedSlug, "page_move", {
      ...actor,
      page: fixture.pageId,
      last_revision_id: movedState.revision,
      new_slug: slug,
      revision_comments: "Recover disposable denial fixture"
    })
    return
  }
  if (action === "delete" && current.type === "missing") {
    /** @type {{ page_id: number; slug: string }[]} */
    const deleted = await rpc(
      admin.context.request,
      admin.token,
      slug,
      "page_get_deleted",
      {
        site_id: siteId,
        slug
      }
    )
    assert.ok(
      deleted.some((entry) => entry.page_id === fixture.pageId && entry.slug === slug)
    )
    await rpc(admin.context.request, admin.token, slug, "page_restore", {
      ...actor,
      page_id: fixture.pageId,
      revision_comments: "Recover disposable denial fixture"
    })
    return
  }
  assert.equal(current.type, "found", "unexpected missing fixture page")
  if (current.type !== "found") assert.fail("fixture page missing")
  assert.equal(current.data.page.page_id, fixture.pageId)
  const now = fingerprint(current.data, await readParents(admin, slug, fixture.pageId))
  assert.equal(now.sourceHash, baseline.sourceHash, "refuse recovery after source change")
  if (now.layout !== baseline.layout) {
    await rpc(admin.context.request, admin.token, slug, "page_set_layout", {
      ...actor,
      page_id: fixture.pageId,
      layout: baseline.layout
    })
  }
  if (JSON.stringify(now.parents) !== JSON.stringify(baseline.parents)) {
    const add = baseline.parents.filter((parent) => !now.parents.includes(parent))
    const remove = now.parents.filter((parent) => !baseline.parents.includes(parent))
    await rpc(admin.context.request, admin.token, slug, "parent_update", {
      site_id: siteId,
      child: fixture.pageId,
      user_id: actor.user_id,
      add,
      remove
    })
  }
  if (JSON.stringify(now.tags) !== JSON.stringify(baseline.tags)) {
    await rpc(admin.context.request, admin.token, slug, "page_edit", {
      ...actor,
      page: fixture.pageId,
      last_revision_id: now.revision,
      revision_comments: "Recover disposable denial fixture tags",
      tags: baseline.tags
    })
  }
  if (action === "blockSet") {
    const block = await rpc(admin.context.request, admin.token, slug, "page_block_get", {
      page: fixture.pageId
    })
    if (block.blocked !== originalBlocked) {
      await rpc(admin.context.request, admin.token, slug, "page_block_set", {
        page: fixture.pageId,
        blocked: originalBlocked,
        ip_address: "127.0.0.1"
      })
    }
  }
}

/**
 * @param {Actor} admin @param {Actor} observer @param {ObserverGuard}
 *   observerGuard @param {Fixture} fixture @param {Fingerprint} baseline
 * @param {Fingerprint} destination @param {boolean} originalBlocked
 * @param {DenialCase} entry
 */
async function assertDenied(
  admin,
  observer,
  observerGuard,
  fixture,
  baseline,
  destination,
  originalBlocked,
  entry
) {
  const { action, body, format } = entry
  let result
  let observedChange = false
  try {
    result = await postAction(
      observer.page,
      observerGuard,
      fixture.sourceSlug,
      action,
      body,
      format
    )
  } finally {
    /** @type {PageView} */
    const current = await rpc(
      admin.context.request,
      admin.token,
      fixture.sourceSlug,
      "page_view",
      {
        site_id: siteId,
        locales: ["en"],
        session_token: admin.token,
        route: { slug: fixture.sourceSlug, extra: "" }
      }
    )
    const changed =
      current.type !== "found" ||
      JSON.stringify(await readFingerprint(admin, fixture.sourceSlug, fixture.pageId)) !==
        JSON.stringify(baseline)
    if (changed) {
      observedChange = true
      await recoverPage(admin, fixture, baseline, originalBlocked, action, current)
    } else {
      const blocked = await rpc(
        admin.context.request,
        admin.token,
        fixture.sourceSlug,
        "page_block_get",
        { page: fixture.pageId }
      )
      if (blocked.blocked !== originalBlocked) {
        observedChange = true
        await recoverPage(admin, fixture, baseline, originalBlocked, action, current)
      }
    }
  }
  assert.equal(observedChange, false, `${action} unexpectedly changed persisted state`)
  assert.deepEqual(
    result,
    { status: 200, type: "failure", actionStatus: 403 },
    `${action} denial envelope`
  )
  assert.deepEqual(
    await readFingerprint(admin, fixture.sourceSlug, fixture.pageId),
    baseline,
    `${action} source, revision, slug, layout, tags, parents unchanged`
  )
  assert.deepEqual(
    await readFingerprint(admin, fixture.destinationSlug, fixture.destinationPageId),
    destination,
    `${action} destination unchanged`
  )
  const block = await rpc(
    admin.context.request,
    admin.token,
    fixture.sourceSlug,
    "page_block_get",
    { page: fixture.pageId }
  )
  assert.equal(block.blocked, originalBlocked, `${action} Block state unchanged`)
}

/** @param {Actor} admin @param {Fixture} fixture */
async function readFileLists(admin, fixture) {
  /** @type {[string, number][]} */
  const pages = [
    [fixture.sourceSlug, fixture.pageId],
    [fixture.destinationSlug, fixture.destinationPageId]
  ]
  /**
   * @type {{
   *   slug: string
   *   pageId: number
   *   deleted: boolean
   *   files: PageFile[]
   * }[]}
   */
  const lists = []
  for (const [slug, pageId] of pages) {
    for (const deleted of [false, true]) {
      const files = await listFiles(
        admin.context.request,
        admin.token,
        slug,
        pageId,
        deleted
      )
      lists.push({
        slug,
        pageId,
        deleted,
        files: files.sort((a, b) => a.file_id - b.file_id)
      })
    }
  }
  return lists
}

/**
 * @param {Actor} admin @param {Fixture} fixture @param {FileLists} before
 * @param {string} name
 */
async function findNewFile(admin, fixture, before, name) {
  const source = (await readFileLists(admin, fixture)).find(
    (list) => list.pageId === fixture.pageId && !list.deleted
  )
  const original = before.find((list) => list.pageId === fixture.pageId && !list.deleted)
  assert.ok(source && original)
  const originalIds = new Set(original.files.map((file) => file.file_id))
  const created = source.files.filter(
    (file) => file.name === name && !originalIds.has(file.file_id)
  )
  assert.ok(created.length <= 1, "only one new named file may be owned")
  return created[0] ?? null
}

/**
 * @param {Actor} admin @param {Fixture} fixture @param {FileLists} before
 * @param {AdminGuard} guard @param {string} name @param {Buffer} bytes
 */
async function uploadDisposableFile(admin, fixture, before, guard, name, bytes) {
  const pane = await openFiles(admin.page, fixture.sourceSlug)
  await pane.locator(".upload-file, .buttons input[value='Upload']").click()
  const form = pane.locator("#file-upload")
  await form.locator('[name="file"]').setInputFiles({
    name,
    mimeType: "text/plain",
    buffer: bytes
  })
  await form.locator('[name="name"]').fill(name)
  await clickMutation(
    admin.page,
    guard,
    fixture.sourceSlug,
    "fileUpload",
    form.locator('[type="submit"]'),
    null,
    name
  )
  const created = await findNewFile(admin, fixture, before, name)
  assert.ok(created, "admin upload must create unique disposable file")
  return created
}

/**
 * @param {Actor} admin @param {Fixture} fixture @param {FileLists} before
 * @param {number | null} fileId
 */
async function assertOriginalFiles(admin, fixture, before, fileId) {
  const current = await readFileLists(admin, fixture)
  for (let index = 0; index < before.length; index++) {
    assert.deepEqual(
      current[index].files.filter((file) => file.file_id !== fileId),
      before[index].files,
      `original files unchanged on ${before[index].slug} deleted=${before[index].deleted}`
    )
  }
  return current
}

/**
 * @param {Actor} admin @param {Fixture} fixture @param {FileLists} before
 * @param {PageFile} created @param {Buffer} bytes
 */
async function assertDisposableFile(admin, fixture, before, created, bytes) {
  const lists = await assertOriginalFiles(admin, fixture, before, created.file_id)
  const owned = lists.flatMap((list) =>
    list.files.filter((file) => file.file_id === created.file_id)
  )
  assert.equal(owned.length, 1, "disposable file remains on source only")
  assert.deepEqual(owned[0], created, "file metadata and revision unchanged")
  const current = await readFileBytes(
    admin.context.request,
    admin.token,
    fixture.sourceSlug,
    fixture.pageId,
    created.file_id
  )
  assert.deepEqual(current.bytes, bytes, "disposable file bytes unchanged")
}

/**
 * @param {Actor} admin @param {Fixture} fixture @param {FileLists} before
 * @param {string} name @param {number | null} fileId
 */
async function tombstoneDisposableFile(admin, fixture, before, name, fileId) {
  const lists = await readFileLists(admin, fixture)
  const originalIds = new Set(
    before.flatMap((list) => list.files.map((file) => file.file_id))
  )
  const candidates = lists.flatMap((list) =>
    list.files
      .filter((file) => file.name === name && !originalIds.has(file.file_id))
      .map((file) => ({ ...file, slug: list.slug }))
  )
  if (fileId !== null) {
    assert.ok(candidates.length > 0, "created file must remain on sacrificial pages")
    assert.ok(candidates.every((file) => file.file_id === fileId))
  }
  assert.ok(candidates.length <= 2, "ambiguous disposable file state")
  const active = candidates.find((file) => file.revision_type !== "delete")
  if (active) {
    assert.ok(
      active.page_id === fixture.pageId || active.page_id === fixture.destinationPageId,
      "cleanup restricted to two sacrificial pages"
    )
    const session = await rpc(
      admin.context.request,
      admin.token,
      active.slug,
      "session_get",
      [admin.token]
    )
    assert.ok(Number.isSafeInteger(session.user_id))
    await rpc(admin.context.request, admin.token, active.slug, "file_delete", {
      site_id: siteId,
      page_id: active.page_id,
      user_id: session.user_id,
      file: active.file_id,
      last_revision_id: active.revision_id,
      revision_comments: "Tombstone disposable observer-denial file"
    })
  }
  await assertOriginalFiles(admin, fixture, before, fileId)
}

test(
  "observer cannot mutate a newly uploaded disposable file",
  { timeout: 120_000 },
  async () => {
    const { fixture, observer } = await readFixtures()
    const { adminPassword, gatewayPassword } = await readPasswords()
    const credentials = {
      httpCredentials: { username: "cobalt", password: gatewayPassword, origin }
    }
    const name = `observer-denial-${randomBytes(8).toString("hex")}.txt`
    const bytes = Buffer.from(`Disposable observer-denial file ${name}\n`)
    const browser = await chromium.launch({
      executablePath: "/usr/bin/chromium",
      headless: true
    })
    try {
      const adminContext = await browser.newContext(credentials)
      const observerContext = await browser.newContext(credentials)
      try {
        const guard = await guardBrowserWrites(adminContext, [
          fixture.sourceSlug,
          fixture.destinationSlug
        ])
        const observerGuard = await guardObserverWrites(observerContext, fixture)
        const admin = await login(adminContext, fixture.username, adminPassword)
        const denied = await login(observerContext, observer.username, observer.password)
        const session = await rpc(
          denied.context.request,
          denied.token,
          fixture.sourceSlug,
          "session_get",
          [denied.token]
        )
        assert.equal(session.user_id, observer.user_id)
        const before = await readFileLists(admin, fixture)
        assert.ok(before.every((list) => list.files.every((file) => file.name !== name)))
        /** @type {number | null} */
        let fileId = null
        try {
          const created = await uploadDisposableFile(
            admin,
            fixture,
            before,
            guard,
            name,
            bytes
          )
          fileId = created.file_id
          assert.ok(Number.isSafeInteger(fileId) && fileId > 0)
          observerGuard.setFileId(fileId)
          await assertDisposableFile(admin, fixture, before, created, bytes)
          const sourcePage = await readFingerprint(
            admin,
            fixture.sourceSlug,
            fixture.pageId
          )
          const destinationPage = await readFingerprint(
            admin,
            fixture.destinationSlug,
            fixture.destinationPageId
          )
          const common = {
            siteId,
            pageId: fixture.pageId,
            fileId,
            lastRevisionId: created.revision_id
          }
          /** @type {DenialCase[]} */
          const cases = [
            { action: "fileDelete", body: { ...common, comments: "" }, format: "json" },
            { action: "fileEdit", body: { ...common, name, comments: "" } },
            {
              action: "fileMove",
              body: {
                ...common,
                destinationPage: fixture.destinationSlug,
                name,
                comments: ""
              }
            }
          ]
          for (const entry of cases) {
            const response = await postAction(
              denied.page,
              observerGuard,
              fixture.sourceSlug,
              entry.action,
              entry.body,
              entry.format
            )
            await assertDisposableFile(admin, fixture, before, created, bytes)
            assert.deepEqual(
              await readFingerprint(admin, fixture.sourceSlug, fixture.pageId),
              sourcePage,
              `${entry.action} source page unchanged`
            )
            assert.deepEqual(
              await readFingerprint(
                admin,
                fixture.destinationSlug,
                fixture.destinationPageId
              ),
              destinationPage,
              `${entry.action} destination page unchanged`
            )
            assert.deepEqual(
              response,
              { status: 200, type: "failure", actionStatus: 403 },
              `${entry.action} denial envelope`
            )
          }
        } finally {
          await tombstoneDisposableFile(admin, fixture, before, name, fileId)
        }
        guard.assertConsumed()
        observerGuard.assertConsumed()
      } finally {
        await observerContext.close()
        await adminContext.close()
      }
    } finally {
      await browser.close()
    }
  }
)

test(
  "observer page actions deny writes without changing sacrificial pages",
  { timeout: 120_000 },
  async () => {
    const { fixture, observer } = await readFixtures()
    const { adminPassword, gatewayPassword } = await readPasswords()
    const credentials = {
      httpCredentials: { username: "cobalt", password: gatewayPassword, origin }
    }
    const browser = await chromium.launch({
      executablePath: "/usr/bin/chromium",
      headless: true
    })
    try {
      const adminContext = await browser.newContext(credentials)
      const observerContext = await browser.newContext(credentials)
      try {
        const adminGuard = await guardBrowserWrites(adminContext, [
          fixture.sourceSlug,
          fixture.destinationSlug
        ])
        const observerGuard = await guardObserverWrites(observerContext, fixture)
        const admin = await login(adminContext, fixture.username, adminPassword)
        const denied = await login(observerContext, observer.username, observer.password)
        const session = await rpc(
          denied.context.request,
          denied.token,
          fixture.sourceSlug,
          "session_get",
          [denied.token]
        )
        assert.equal(session.user_id, observer.user_id, "observer browser identity")
        const permission = await rpc(
          denied.context.request,
          denied.token,
          fixture.sourceSlug,
          "page_edit_permission",
          {}
        )
        assert.equal(permission.can_edit, false, "observer must lack edit permission")
        const baseline = await readFingerprint(admin, fixture.sourceSlug, fixture.pageId)
        const destination = await readFingerprint(
          admin,
          fixture.destinationSlug,
          fixture.destinationPageId
        )
        const block = await rpc(
          admin.context.request,
          admin.token,
          fixture.sourceSlug,
          "page_block_get",
          {
            page: fixture.pageId
          }
        )
        assert.equal(typeof block.blocked, "boolean", "Block baseline required")
        assert.equal(
          (
            await rpc(
              admin.context.request,
              admin.token,
              fixture.sourceSlug,
              "page_view",
              {
                site_id: siteId,
                locales: ["en"],
                session_token: admin.token,
                route: { slug: fixture.movedSlug, extra: "" }
              }
            )
          ).type,
          "missing",
          "recovery slug must be vacant"
        )
        const common = {
          siteId,
          pageId: fixture.pageId,
          lastRevisionId: baseline.revision
        }
        /** @type {DenialCase[]} */
        const cases = [
          {
            action: "move",
            body: { ...common, newSlug: fixture.movedSlug, comments: "" }
          },
          { action: "delete", body: { ...common, option: "delete", comments: "" } },
          {
            action: "parentSet",
            body: {
              ...common,
              parents: fixture.destinationSlug,
              addParents: [fixture.destinationSlug],
              removeParents: []
            }
          },
          {
            action: "layout",
            body: {
              ...common,
              layout: baseline.layout === "wikijump" ? "wikidot" : "wikijump"
            }
          },
          {
            action: "setTags",
            body: { changes: "+local-observer-denial" },
            format: "form"
          },
          {
            action: "blockSet",
            body: {
              pageId: fixture.pageId,
              blocked: !block.blocked
            },
            format: "json"
          }
        ]
        for (const entry of cases) {
          await assertDenied(
            admin,
            denied,
            observerGuard,
            fixture,
            baseline,
            destination,
            block.blocked,
            entry
          )
        }
        adminGuard.assertConsumed()
        observerGuard.assertConsumed()
      } finally {
        await observerContext.close()
        await adminContext.close()
      }
    } finally {
      await browser.close()
    }
  }
)
