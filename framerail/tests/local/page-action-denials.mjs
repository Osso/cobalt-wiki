import assert from "node:assert/strict"
import { createHash, randomBytes } from "node:crypto"
import { readFile } from "node:fs/promises"
import { test, expect, chromium } from "@playwright/test"
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
const adminPasswordPath = "/home/osso/.local/share/cobalt-wiki/local-full/admin-password"

async function readFixtures() {
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

async function readPage(actor, slug, pageId) {
  const view = await rpc(actor.context.request, actor.token, slug, "page_view", {
    site_id: siteId,
    locales: ["en"],
    session_token: actor.token,
    route: { slug, extra: "" }
  })
  assert.equal(view.type, "found", `${slug} must remain live`)
  assert.equal(view.data.page.page_id, pageId, "only manifest ID allowed")
  assert.equal(view.data.page.slug, slug)
  return view.data
}

async function readParents(actor, slug, pageId) {
  return rpc(actor.context.request, actor.token, slug, "parent_get_all", {
    site_id: siteId,
    page: pageId
  })
}

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

async function readFingerprint(actor, slug, pageId) {
  const page = await readPage(actor, slug, pageId)
  const parents = await readParents(actor, slug, pageId)
  return fingerprint(page, parents)
}

async function postAction(page, slug, action, body, format = "superform") {
  const encoded = format === "superform" ? stringify(body) : body
  return page.evaluate(
    async ({ slug, action, encoded, format }) => {
      const payload =
        format === "json"
          ? JSON.stringify(encoded)
          : new URLSearchParams(
              format === "superform" ? { __superform_json: encoded } : encoded
            )
      const headers = { accept: "application/json", "x-sveltekit-action": "true" }
      if (format === "json") headers["content-type"] = "application/json"
      const response = await fetch(`/${slug}?/${action}`, {
        method: "POST",
        headers,
        body: payload
      })
      const result = await response.json()
      return { status: response.status, type: result.type, actionStatus: result.status }
    },
    { slug, action, encoded, format }
  )
}

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

async function assertDenied(
  admin,
  observer,
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
    result = await postAction(observer.page, fixture.sourceSlug, action, body, format)
  } finally {
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

async function readFileLists(admin, fixture) {
  const pages = [
    [fixture.sourceSlug, fixture.pageId],
    [fixture.destinationSlug, fixture.destinationPageId]
  ]
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
    const name = `observer-denial-${randomBytes(8).toString("hex")}.txt`
    const bytes = Buffer.from(`Disposable observer-denial file ${name}\n`)
    const browser = await chromium.launch({
      executablePath: "/usr/bin/chromium",
      headless: true
    })
    try {
      const adminContext = await browser.newContext()
      const observerContext = await browser.newContext()
      try {
        const guard = await guardBrowserWrites(adminContext, [
          fixture.sourceSlug,
          fixture.destinationSlug
        ])
        const admin = await login(
          adminContext,
          fixture.username,
          (await readFile(adminPasswordPath, "utf8")).trim()
        )
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
    const browser = await chromium.launch({
      executablePath: "/usr/bin/chromium",
      headless: true
    })
    try {
      const adminContext = await browser.newContext()
      const observerContext = await browser.newContext()
      try {
        const admin = await login(
          adminContext,
          fixture.username,
          (await readFile(adminPasswordPath, "utf8")).trim()
        )
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
              blocked: !baseline.blocked
            },
            format: "json"
          }
        ]
        for (const entry of cases) {
          await assertDenied(
            admin,
            denied,
            fixture,
            baseline,
            destination,
            block.blocked,
            entry
          )
        }
      } finally {
        await observerContext.close()
        await adminContext.close()
      }
    } finally {
      await browser.close()
    }
  }
)
