import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { readFile, writeFile } from "node:fs/promises"
import { test } from "node:test"
import { chromium, expect } from "@playwright/test"

const origin = "http://127.0.0.1:3090"
const backend = "http://127.0.0.1:2749/jsonrpc"
const directory = "/home/osso/.local/share/cobalt-wiki/local-full/watching-proof"
const fixturePath = `${directory}/browser-fixture.json`
const statePath = `${directory}/created-page.json`
const proofPath = `${directory}/ui-proof.json`
const adminPasswordPath = "/home/osso/.local/share/cobalt-wiki/local-full/admin-password"
const siteId = 6000000
const title = "Watcher UI acceptance"
const before = "Before watcher UI change."
const after = "After watcher UI change."
const allowedRpc = new Set([
  "login",
  "session_get",
  "page_get",
  "page_create",
  "page_edit",
  "watching_activity",
  "watching_preferences_get",
  "watching_preferences_set",
  "watching_subscriptions",
  "watching_subscription_set"
])

const hash = (value) => createHash("sha256").update(JSON.stringify(value)).digest("hex")

async function readFixture() {
  const fixture = JSON.parse(await readFile(fixturePath, "utf8"))
  assert.equal(fixture.sacrificial, true)
  assert.equal(fixture.site_id, siteId)
  assert.equal(fixture.origin, origin)
  assert.equal(fixture.page_slug, "home:start")
  assert.equal(fixture.email_must_remain_disabled, true)
  assert.ok(Number.isSafeInteger(fixture.user_id) && fixture.user_id > 0)
  assert.ok(typeof fixture.username === "string" && fixture.username.length > 0)
  assert.ok(typeof fixture.password === "string" && fixture.password.length > 0)
  assert.match(
    fixture.page_slug_for_event ?? "",
    /^local-watch-proof:[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/i
  )
  return fixture
}

async function rpc(request, method, params, token, pageSlug) {
  assert.ok(allowedRpc.has(method), "RPC method must be on the acceptance allowlist")
  const headers = { "X-Deepwell-Site-Id": String(siteId) }
  if (token) headers["X-Deepwell-Session-Token"] = token
  if (pageSlug) headers["X-Deepwell-Page"] = pageSlug
  const response = await request.post(backend, {
    headers,
    data: { jsonrpc: "2.0", id: 1, method, params },
    timeout: 10000
  })
  assert.equal(response.status(), 200, `${method} transport status`)
  const result = await response.json()
  assert.ok(
    !result.error,
    `${method} RPC must succeed (code ${result.error?.code ?? "?"})`
  )
  return result.result
}

function guardBrowserPosts(context) {
  return context.route("**/*", (route) => {
    const request = route.request()
    if (request.method() !== "POST") return route.continue()
    const url = new URL(request.url())
    const allowed =
      url.origin === origin &&
      ((url.pathname === "/-/login" && ["", "?/login"].includes(url.search)) ||
        (url.pathname === "/home:start" && url.search === "?/watching") ||
        (url.pathname === "/-/settings" &&
          ["?/watching", "?/unwatch"].includes(url.search)))
    if (!allowed) throw new Error("browser POST outside login/watch/settings allowlist")
    return route.continue()
  })
}

async function loginReader(context, fixture) {
  const page = await context.newPage()
  await page.goto(`${origin}/-/login`, { waitUntil: "networkidle" })
  await expect(page.locator("#login")).toBeVisible()
  await page.locator('#login [name="nameOrEmail"]').fill(fixture.username)
  await page.locator('#login [name="password"]').fill(fixture.password)
  await page.locator('#login button[type="submit"]').click()
  await expect(page.locator("#login")).toHaveCount(0)
  const cookie = (await context.cookies()).find(
    (entry) => entry.name === "wikijump_token"
  )
  assert.ok(cookie, "reader browser must be authenticated")
  const token = decodeURIComponent(cookie.value)
  const session = await rpc(context.request, "session_get", [token])
  assert.equal(session?.user_id, fixture.user_id, "reader must match sacrificial fixture")
  assert.equal(session.restricted, false)
  return { page, token }
}

async function assertEmailOff(request, token, autoWatch) {
  const preferences = await rpc(request, "watching_preferences_get", {}, token)
  assert.deepEqual(preferences, { email_enabled: false, auto_watch: autoWatch })
}

async function assertSettings(page, scopes) {
  await page.goto(`${origin}/-/settings`, { waitUntil: "networkidle" })
  const rows = page
    .locator("h2")
    .filter({ hasText: "My subscriptions" })
    .locator("xpath=following-sibling::ul[1]/li")
  await expect(rows).toHaveCount(scopes.length)
  for (const entry of scopes) {
    await expect(
      rows.filter({ hasText: `${entry.scope} #${entry.target_id}` })
    ).toHaveCount(1)
  }
  return rows
}

async function saveAutoWatch(page, enabled) {
  const form = page.locator('form[action="?/watching"]')
  await expect(form.getByLabel("Email me about watched pages")).not.toBeChecked()
  const auto = form.getByLabel("Automatically watch pages after I edit them")
  if (enabled) await auto.check()
  else await auto.uncheck()
  await form.getByRole("button", { name: "Save watching preferences" }).click()
  await expect(page.getByText("Watching preferences saved.")).toBeVisible()
  await page.reload({ waitUntil: "networkidle" })
  await expect(form.getByLabel("Email me about watched pages")).not.toBeChecked()
  if (enabled) await expect(auto).toBeChecked()
  else await expect(auto).not.toBeChecked()
}

async function showWatchControls(page) {
  const controls = page.locator('[aria-label="Watching"]')
  if (!(await controls.count())) await page.locator("#more-options-button").click()
  await expect(controls).toBeVisible()
}

async function watchThree(page, request, token, fixture, targets) {
  await page.goto(`${origin}/${fixture.page_slug}`, { waitUntil: "networkidle" })
  const controls = page.locator('[aria-label="Watching"]')
  const labels = ["site", "category", "page"]
  for (const label of labels) {
    await showWatchControls(page)
    await controls.getByRole("button", { name: `Watch this ${label}` }).click()
    await page.waitForLoadState("networkidle")
    await showWatchControls(page)
    await expect(
      controls.getByRole("button", { name: `Unwatch this ${label}` })
    ).toBeVisible()
    await page.reload({ waitUntil: "networkidle" })
    await showWatchControls(page)
    await expect(
      controls.getByRole("button", { name: `Unwatch this ${label}` })
    ).toBeVisible()
    await assertEmailOff(request, token, false)
  }
  const subscriptions = await rpc(
    request,
    "watching_subscriptions",
    { site_id: siteId },
    token
  )
  assert.deepEqual(subscriptions.map((item) => item.scope).sort(), labels.sort())
  for (const entry of subscriptions) assert.equal(entry.target_id, targets[entry.scope])
  return subscriptions
}

async function loginAdmin(request, fixture, password) {
  const login = await rpc(request, "login", {
    name_or_email: "cobalt-import",
    password,
    ip_address: "127.0.0.1",
    user_agent: "cobalt-local-watching-acceptance"
  })
  assert.equal(login.needs_mfa, false)
  assert.ok(login.session_token)
  const session = await rpc(request, "session_get", [login.session_token])
  assert.ok(Number.isSafeInteger(session?.user_id) && session.user_id > 0)
  assert.notEqual(session.user_id, fixture.user_id, "actor must differ from reader")
  assert.equal(session.restricted, false)
  return { token: login.session_token, userId: session.user_id }
}

async function createAndEdit(request, fixture, actor, readerPage, readerToken) {
  const slug = fixture.page_slug_for_event
  const missing = await rpc(
    request,
    "page_get",
    { site_id: siteId, page: slug },
    actor.token
  )
  assert.equal(missing, null, "event slug already exists: reconcile rather than replay")
  // Exclusive intent prevents a second run after an ambiguous create response.
  await writeFile(statePath, JSON.stringify({ slug, site_id: siteId, phase: "intent" }), {
    flag: "wx",
    mode: 0o600
  })
  const base = {
    site_id: siteId,
    user_id: actor.userId,
    ip_address: "127.0.0.1",
    revision_comments: "Synthetic watcher acceptance",
    do_not_notify_watchers: false
  }
  const created = await rpc(
    request,
    "page_create",
    {
      ...base,
      slug,
      title,
      wikitext: before,
      alt_title: null,
      tags: [],
      layout: null
    },
    actor.token,
    slug
  )
  assert.equal(created.slug, slug)
  assert.ok(Number.isSafeInteger(created.page_id) && created.page_id > 0)
  assert.ok(Number.isSafeInteger(created.revision_id) && created.revision_id > 0)
  const state = {
    slug,
    site_id: siteId,
    page_id: created.page_id,
    create_revision_id: created.revision_id
  }
  await writeFile(statePath, JSON.stringify(state), { mode: 0o600 })
  await assertEmailOff(request, readerToken, false)
  await waitForActivity(readerPage, 1)
  const current = await rpc(
    request,
    "page_get",
    {
      site_id: siteId,
      page: slug,
      details: { wikitext: true }
    },
    actor.token
  )
  assert.equal(current?.page_id, created.page_id)
  assert.equal(current?.revision_id, created.revision_id)
  assert.equal(current?.wikitext, before)
  const edited = await rpc(
    request,
    "page_edit",
    {
      ...base,
      page: created.page_id,
      last_revision_id: current.revision_id,
      wikitext: after
    },
    actor.token,
    String(created.page_id)
  )
  assert.ok(edited?.revision_id > created.revision_id, "edit must advance revision")
  state.edit_revision_id = edited.revision_id
  await writeFile(statePath, JSON.stringify(state), { mode: 0o600 })
  await assertEmailOff(request, readerToken, false)
  await waitForActivity(readerPage, 2)
  return state
}

async function waitForActivity(page, count) {
  for (let attempt = 0; attempt < 20; attempt++) {
    await page.goto(`${origin}/-/activity`, { waitUntil: "networkidle" })
    const entries = page.getByRole("link", { name: title, exact: true })
    if ((await entries.count()) === count) return
    await page.waitForTimeout(1000)
  }
  assert.fail(`synthetic Activity did not reach ${count} entries within 20 attempts`)
}

async function assertActivityDetails(page, request, token, state) {
  const activity = await rpc(
    request,
    "watching_activity",
    { site_id: siteId, limit: 20 },
    token
  )
  assert.equal(activity.items.length, 2, "one deduplicated notification per change")
  assert.deepEqual(
    activity.items.map((item) => item.event_type),
    ["edit", "create"]
  )
  assert.deepEqual(
    activity.items.map((item) => item.revision_id),
    [state.edit_revision_id, state.create_revision_id]
  )
  assert.ok(
    activity.items.every((item) => item.page_id === state.page_id && item.title === title)
  )
  for (const item of activity.items) {
    const link = page.locator(`a[href="/-/activity?event=${item.event_id}"]`)
    await expect(link).toHaveText(title)
    await link.click()
    await expect(page.locator("article h2")).toHaveText(title)
    await expect(page.locator("article pre").last()).toHaveText(
      item.event_type === "create" ? before : after
    )
    if (item.event_type === "edit") {
      await expect(page.locator("article pre").first()).toHaveText(before)
    } else {
      await expect(page.locator("article h3", { hasText: "Before" })).toHaveCount(0)
    }
    await page.goto(`${origin}/-/activity`, { waitUntil: "networkidle" })
  }
  return activity.items.map((item) => item.event_id)
}

async function unwatchViaSettings(page, request, token, subscriptions) {
  for (const entry of subscriptions) {
    const rows = await assertSettings(page, subscriptions)
    const row = rows.filter({ hasText: `${entry.scope} #${entry.target_id}` })
    await row.getByRole("button", { name: "Unwatch" }).click()
    await page.reload({ waitUntil: "networkidle" })
    await expect(page.getByText(`${entry.scope} #${entry.target_id}`)).toHaveCount(0)
    subscriptions = subscriptions.filter((item) => item.scope !== entry.scope)
  }
  await assertSettings(page, [])
  await expect(page.getByText("No subscriptions on this site.")).toBeVisible()
  assert.deepEqual(
    await rpc(request, "watching_subscriptions", { site_id: siteId }, token),
    []
  )
}

async function restoreReader(request, token, targets) {
  const errors = []
  try {
    await rpc(
      request,
      "watching_preferences_set",
      {
        email_enabled: false,
        auto_watch: false
      },
      token
    )
  } catch {
    errors.push("preferences")
  }
  let subscriptions = []
  try {
    subscriptions = await rpc(
      request,
      "watching_subscriptions",
      { site_id: siteId },
      token
    )
  } catch {
    errors.push("list-subscriptions")
  }
  for (const entry of subscriptions) {
    if (targets?.[entry.scope] !== entry.target_id) continue
    try {
      await rpc(
        request,
        "watching_subscription_set",
        {
          site_id: siteId,
          scope: entry.scope,
          target_id: entry.target_id,
          watching: false
        },
        token
      )
    } catch {
      errors.push(`unwatch-${entry.scope}`)
    }
  }
  assert.deepEqual(errors, [], "reader cleanup must complete")
}

test("local reader watches UI, sees synthetic changes, and restores account", async () => {
  let stage = "fixture"
  let browser
  let reader
  let token
  let subscriptions = []
  let targets
  let error
  let proof
  try {
    const fixture = await readFixture()
    const gatewayFile = process.env.COBALT_LOCAL_PASSWORD_FILE
    assert.ok(gatewayFile, "COBALT_LOCAL_PASSWORD_FILE required")
    const gatewayPassword = (await readFile(gatewayFile, "utf8")).trim()
    browser = await chromium.launch({
      executablePath: "/usr/bin/chromium",
      headless: true
    })
    const credentials = { username: "cobalt", password: gatewayPassword, origin }
    const anonymous = await browser.newContext({ httpCredentials: credentials })
    reader = await browser.newContext({ httpCredentials: credentials })
    await guardBrowserPosts(anonymous)
    await guardBrowserPosts(reader)
    stage = "anonymous"
    const anonymousPage = await anonymous.newPage()
    await anonymousPage.goto(`${origin}/${fixture.page_slug}`, {
      waitUntil: "networkidle"
    })
    await anonymousPage.locator("#more-options-button").click()
    await expect(anonymousPage.locator('[aria-label="Watching"]')).toHaveCount(0)
    await expect(anonymousPage.getByRole("button", { name: /^Watch this / })).toHaveCount(
      0
    )
    await anonymous.close()
    stage = "login"
    const signedIn = await loginReader(reader, fixture)
    token = signedIn.token
    const page = signedIn.page
    stage = "initial"
    const initialSubscriptions = await rpc(
      reader.request,
      "watching_subscriptions",
      {
        site_id: siteId
      },
      token
    )
    assert.deepEqual(initialSubscriptions, [])
    await assertEmailOff(reader.request, token, false)
    const initialActivity = await rpc(
      reader.request,
      "watching_activity",
      {
        site_id: siteId,
        limit: 20
      },
      token
    )
    assert.deepEqual(initialActivity.items, [])
    const home = await rpc(
      reader.request,
      "page_get",
      {
        site_id: siteId,
        page: fixture.page_slug
      },
      token
    )
    assert.equal(home?.site_id, siteId)
    assert.equal(home?.slug, fixture.page_slug)
    targets = { site: siteId, category: home.page_category_id, page: home.page_id }
    await page.goto(`${origin}/-/activity`, { waitUntil: "networkidle" })
    await expect(page.getByText("No watched page changes here.")).toBeVisible()
    await assertSettings(page, [])
    stage = "watch"
    subscriptions = await watchThree(page, reader.request, token, fixture, targets)
    await assertSettings(page, subscriptions)
    stage = "preferences"
    await saveAutoWatch(page, true)
    await assertEmailOff(reader.request, token, true)
    await saveAutoWatch(page, false)
    await assertEmailOff(reader.request, token, false)
    stage = "actor"
    const adminPassword = (await readFile(adminPasswordPath, "utf8")).trim()
    const actor = await loginAdmin(reader.request, fixture, adminPassword)
    stage = "create-edit"
    const state = await createAndEdit(reader.request, fixture, actor, page, token)
    stage = "activity-detail"
    const eventIds = await assertActivityDetails(page, reader.request, token, state)
    stage = "unwatch"
    await unwatchViaSettings(page, reader.request, token, subscriptions)
    subscriptions = []
    await assertEmailOff(reader.request, token, false)
    stage = "proof"
    proof = {
      subscription_count: 3,
      event_count: eventIds.length,
      event_hash: hash(eventIds),
      revision_hash: hash([state.create_revision_id, state.edit_revision_id])
    }
  } catch (caught) {
    await writeFile(
      `${directory}/ui-failure.json`,
      JSON.stringify({
        stage,
        error: String(caught),
        watching: reader
          ? await reader
              .pages()
              .at(-1)
              ?.locator('[aria-label="Watching"]')
              .innerText()
              .catch(() => "unavailable")
          : null
      }),
      { mode: 0o600 }
    )
    error = new Error(
      `watching acceptance failed at ${stage}; inspect protected state and reconcile before rerun`
    )
  } finally {
    if (reader && token) {
      try {
        await restoreReader(reader.request, token, targets)
      } catch {
        error = new Error(`watching acceptance cleanup failed after ${stage}`)
      }
    }
    if (browser) await browser.close()
  }
  if (error) throw error
  await writeFile(proofPath, JSON.stringify(proof), { mode: 0o600 })
})
