import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const watching = await vite.ssrLoadModule("/src/lib/server/load/watching.ts")
const activity = await vite.ssrLoadModule("/src/routes/[x+2d]/activity/+page.server.ts")
const unsubscribe = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/watching-unsubscribe/+page.server.ts"
)
const editor = await vite.ssrLoadModule("/src/routes/[slug]/[...extra]/+page.server.ts")
const homepage = await vite.ssrLoadModule("/src/routes/+page.server.ts")
const { default: WatchControls } = await vite.ssrLoadModule(
  "/src/lib/component/WatchControls.svelte"
)
const { default: ActivityPage } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/activity/+page.svelte"
)
const { default: UnsubscribePage } = await vite.ssrLoadModule(
  "/src/routes/[x+2d]/watching-unsubscribe/+page.svelte"
)
const { render } = await vite.ssrLoadModule("svelte/server")
const headers = { "X-Wikijump-Site-Id": "6000011", "X-Wikijump-Site-Slug": "cobalt" }
type Rpc = { method: string; params: Record<string, unknown>; id: number }

async function withRpc<T>(reply: (rpc: Rpc) => object, callback: () => Promise<T>) {
  const previous = globalThis.fetch
  const calls: Rpc[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as Rpc
    calls.push(rpc)
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...reply(rpc) }))
  }
  try {
    return { value: await callback(), calls }
  } finally {
    globalThis.fetch = previous
  }
}

function event(
  path: string,
  session: string | null = "secret",
  fields?: Record<string, string>
) {
  return {
    request: new Request(`http://local.test${path}`, {
      headers,
      ...(fields ? { method: "POST", body: new URLSearchParams(fields) } : {})
    }),
    url: new URL(`http://local.test${path}`),
    cookies: { get: () => session ?? undefined },
    parent: async () => ({
      site: { layout: null },
      locales: ["en"],
      user_session: session ? { user: { user_id: 42 } } : null
    })
  }
}

const subscriptions = [
  { scope: "site", target_id: 6000011 },
  { scope: "page", target_id: 12 }
]

test("anonymous page skips subscription RPC and renders no watch controls", async () => {
  const { value, calls } = await withRpc(
    () => {
      throw Error("unexpected RPC")
    },
    () => watching.loadPageWatching(6000011, { page_id: 12, page_category_id: 7 }, null)
  )
  assert.equal(value, null)
  assert.equal(calls.length, 0)
  assert.doesNotMatch(
    render(WatchControls, { props: { watching: value } }).body,
    /Watch this/
  )
})

test("existing signed-in page shows current subscription state for site/category/page", async () => {
  const { value, calls } = await withRpc(
    () => ({ result: subscriptions }),
    () =>
      watching.loadPageWatching(6000011, { page_id: 12, page_category_id: 7 }, "secret")
  )
  assert.deepEqual(
    calls.map((call) => [call.method, call.params]),
    [["watching_subscriptions", { site_id: 6000011 }]]
  )
  const body = render(WatchControls, { props: { watching: value } }).body
  assert.match(body, /Unwatch this site/)
  assert.match(body, /Watch this category/)
  assert.match(body, /Unwatch this page/)
  assert.match(body, /name="target_id"[^>]*value="7"/)
})

function replyToPageLoad(rpc: Rpc) {
  if (rpc.method === "page_view") {
    return {
      result: {
        type: "found",
        data: {
          options: {},
          redirect_page: null,
          wikitext: "content",
          compiled_body_html: "<p>content</p>",
          page: {
            page_id: 12,
            page_category_id: 7,
            slug: "test",
            created_at: "2026-09-25T12:00:00Z"
          },
          page_revision: { title: "Test", revision_number: 1 },
          attributions: []
        }
      }
    }
  }
  if (rpc.method === "watching_subscriptions") return { result: subscriptions }
  if (rpc.method === "translate") return { result: {} }
  throw Error(`unexpected RPC ${rpc.method}`)
}

test("existing page route reads watches only for authenticated readers", async () => {
  for (const route of [editor, homepage]) {
    for (const session of [null, "secret"]) {
      const { value, calls } = await withRpc(replyToPageLoad, () =>
        route.load({
          ...event("/test", session),
          params: { slug: "test", extra: "" },
          locals: {}
        })
      )
      assert.equal(value.watching === null, session === null)
      assert.equal(
        calls.filter((call) => call.method === "watching_subscriptions").length,
        session ? 1 : 0
      )
    }
    const staleSession = {
      ...event("/test"),
      parent: async () => ({
        site: { layout: null },
        locales: ["en"],
        user_session: null
      }),
      params: { slug: "test", extra: "" },
      locals: {}
    }
    const stale = await withRpc(replyToPageLoad, () => route.load(staleSession))
    assert.equal(stale.value.watching, null)
    assert.equal(
      stale.calls.filter((call) => call.method === "watching_subscriptions").length,
      0
    )
  }
})

test("subscription action uses current site and explicit watch state, never a user id", async () => {
  const { value, calls } = await withRpc(
    () => ({ result: { watching: false } }),
    () =>
      watching.setSubscriptionAction(
        event("/article", "secret", {
          scope: "page",
          target_id: "12",
          watching: "false",
          site_id: "9",
          user_id: "5"
        })
      )
  )
  assert.deepEqual(
    calls.map((call) => [call.method, call.params]),
    [
      [
        "watching_subscription_set",
        { site_id: 6000011, scope: "page", target_id: 12, watching: false }
      ]
    ]
  )
  assert.equal(value.watching, false)
})

test("subscription errors return visible feedback and signed-out mutations do not call RPC", async () => {
  const failed = await withRpc(
    () => ({ error: { code: 4001, message: "Cannot watch this page" } }),
    () =>
      watching.setSubscriptionAction(
        event("/article", "secret", { scope: "page", target_id: "12", watching: "true" })
      )
  )
  assert.equal(failed.value.status, 400)
  assert.match(failed.value.data.message, /Cannot watch this page/)
  const denied = await withRpc(
    () => {
      throw Error("unexpected RPC")
    },
    () =>
      watching.setSubscriptionAction(
        event("/article", null, { scope: "site", target_id: "6000011", watching: "true" })
      )
  )
  assert.equal(denied.value.status, 401)
  assert.equal(denied.calls.length, 0)
})

const first = {
  event_id: 30,
  page_id: 12,
  revision_id: 70,
  previous_revision_id: 69,
  title: "Example",
  slug: "example",
  actor: "Alice",
  created_at: "2026-09-25T12:00:00Z",
  event_type: "edit"
}

test("activity keeps pagination cursor across two backend pages", async () => {
  const reply = (rpc: Rpc) => ({
    result:
      rpc.params.before_event_id === 20
        ? { items: [{ ...first, event_id: 19 }], next_before: null }
        : { items: [first], next_before: 20 }
  })
  const page1 = await withRpc(reply, () => activity.load(event("/-/activity")))
  const page2 = await withRpc(reply, () => activity.load(event("/-/activity?before=20")))
  assert.deepEqual(page1.calls[0].params, { site_id: 6000011, limit: 20 })
  assert.deepEqual(page2.calls[0].params, {
    site_id: 6000011,
    before_event_id: 20,
    limit: 20
  })
  assert.match(render(ActivityPage, { props: { data: page1.value } }).body, /before=20/)
  assert.match(render(ActivityPage, { props: { data: page1.value } }).body, /Example/)
  assert.doesNotMatch(
    render(ActivityPage, { props: { data: page2.value } }).body,
    /Older/
  )
})

test("activity change link loads full escaped rendered text without leaking source", async () => {
  const reply = (rpc: Rpc) =>
    rpc.method === "watching_change"
      ? {
          result: {
            ...first,
            before_text: "Old <b>markup</b>",
            after_text: "New <script>alert(1)</script>"
          }
        }
      : { result: { items: [first], next_before: null } }
  const list = await withRpc(reply, () => activity.load(event("/-/activity")))
  assert.match(
    render(ActivityPage, { props: { data: list.value } }).body,
    /\/-\/activity\?event=30/
  )
  const detail = await withRpc(reply, () => activity.load(event("/-/activity?event=30")))
  assert.deepEqual(
    detail.calls.map((call) => [call.method, call.params]),
    [["watching_change", { event_id: 30 }]]
  )
  const body = render(ActivityPage, { props: { data: detail.value } }).body
  assert.match(body, /Old &lt;b>markup&lt;\/b>/)
  assert.match(body, /New &lt;script>alert\(1\)&lt;\/script>/)
  assert.doesNotMatch(body, /<script>/)
})

test("unavailable activity changes are not shown as an empty feed", async () => {
  await assert.rejects(
    withRpc(
      () => ({ result: null }),
      () => activity.load(event("/-/activity?event=30"))
    ),
    (caught: unknown) =>
      typeof caught === "object" &&
      caught !== null &&
      "status" in caught &&
      caught.status === 404
  )
})

async function assertRawEditSuppression(pageId: number, suppressed: boolean) {
  const fields = {
    pageId: String(pageId),
    siteId: "6000011",
    lastRevisionId: "70",
    title: "Test",
    altTitle: "",
    tags: "",
    comments: "",
    ...(suppressed ? { doNotNotifyWatchers: "true" } : {}),
    wikitext: "Test content"
  }
  const submission = await withRpc(
    (rpc) =>
      rpc.method === "session_get"
        ? { result: { user_id: 42 } }
        : { result: { revision_id: 71, revision_number: 2 } },
    () =>
      editor.actions.edit({
        ...event("/test/edit", "secret", fields),
        params: { slug: "test" },
        locals: {},
        getClientAddress: () => "203.0.113.9"
      })
  )
  assert.equal(submission.value.status, undefined, JSON.stringify(submission.value.data))
  const rpc = submission.calls.find(
    (call) => call.method === (pageId ? "page_edit" : "page_create")
  )
  assert.ok(rpc)
  assert.equal(rpc.params.do_not_notify_watchers, suppressed)
}

test("raw create/edit submissions send explicit suppression boolean", async () => {
  for (const pageId of [0, 12]) {
    for (const suppressed of [false, true]) {
      await assertRawEditSuppression(pageId, suppressed)
    }
  }
})

test("structured create/edit wrapper retains watcher suppression beside form updates", async () => {
  const { pageEdit } = await vite.ssrLoadModule("/src/lib/server/deepwell/page.ts")
  for (const pageId of [0, 12]) {
    const { calls } = await withRpc(
      () => ({ result: { revision_id: 70, revision_number: 2 } }),
      () =>
        pageEdit(
          6000011,
          pageId,
          42,
          "203.0.113.9",
          "test",
          69,
          "",
          undefined,
          "Test",
          "",
          [],
          undefined,
          { name: "value" },
          true,
          { sessionToken: "secret" }
        )
    )
    assert.equal(calls[0].params.do_not_notify_watchers, true)
    assert.deepEqual(calls[0].params.form_updates, { name: "value" })
  }
})

test("unsubscribe GET calls token-only backend once and shows bounded invalid-token message", async () => {
  const good = await withRpc(
    () => ({ result: { unsubscribed: true } }),
    () => unsubscribe.load(event("/-/watching-unsubscribe?token=secret-token", null))
  )
  assert.deepEqual(
    good.calls.map((call) => [call.method, call.params]),
    [["watching_unsubscribe", { token: "secret-token" }]]
  )
  assert.match(
    render(UnsubscribePage, { props: { data: good.value } }).body,
    /Watcher email is disabled/
  )
  const bad = await withRpc(
    () => ({ error: { code: 4000, message: "secret-token is invalid" } }),
    () => unsubscribe.load(event("/-/watching-unsubscribe?token=secret-token", null))
  )
  const body = render(UnsubscribePage, { props: { data: bad.value } }).body
  assert.match(body, /invalid or expired/)
  assert.doesNotMatch(body, /secret-token/)
})
