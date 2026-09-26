import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const printRoute = await vite.ssrLoadModule(
  "/src/routes/printer--friendly/[...path]/+page.server.ts"
)
const { default: PrintPage } = await vite.ssrLoadModule(
  "/src/routes/printer--friendly/[...path]/+page.svelte"
)
const { default: RootLayout } = await vite.ssrLoadModule("/src/routes/+layout.svelte")
const { render } = await vite.ssrLoadModule("svelte/server")
const { readable } = await vite.ssrLoadModule("svelte/store")

function requestContext(data: Record<string, unknown>) {
  const page = {
    url: new URL("https://wiki.example/printer--friendly/guild:welcome"),
    data,
    form: null,
    params: {},
    route: { id: "/printer--friendly/[...path]" },
    state: {},
    status: 200,
    error: null
  }
  return new Map<string, unknown>([
    ["__request__", { page }],
    [
      "__svelte__",
      {
        page: readable(page),
        navigating: readable(null),
        updated: { subscribe: readable(false).subscribe, check: async () => false }
      }
    ]
  ])
}

const pageData = {
  printView: true,
  site: { name: "Cobalt Company", slug: "cobalt-company", layout: "wikidot" },
  page: { slug: "guild:welcome" },
  page_revision: { title: "Welcome & friends" },
  compiled_body_html: "<p>Printable content</p>",
  sourceUrl: "https://wiki.example/guild:welcome",
  internationalization: { "footer-license-unless": "Content licensed CC BY-SA" }
}

test("print page SSR shows title, compiled body, source and existing license without actions", () => {
  const { body } = render(PrintPage, { props: { data: pageData } })
  assert.match(body, /Welcome &amp; friends/)
  assert.match(body, /<p>Printable content<\/p>/)
  assert.match(body, /https:\/\/wiki\.example\/guild:welcome/)
  assert.match(body, /Content licensed CC BY-SA/)
  assert.match(body, /Print the page/)
  assert.doesNotMatch(body, /<form|page-options|editor|sidebar/i)
  const { body: withoutLicense } = render(PrintPage, {
    props: { data: { ...pageData, internationalization: {} } }
  })
  assert.doesNotMatch(withoutLicense, /licensed|license-area/i)
})

test("print root layout SSR omits site chrome, footer, and editing controls", () => {
  const { body } = render(RootLayout, { context: requestContext(pageData) })
  assert.doesNotMatch(
    body,
    /header-wrap|side-bar|footer|page-options-container|SearchBox|Cobalt Company/
  )
})

test("print route uses page_view with same cookie, locale, and site for nested path", async () => {
  assert.equal(printRoute.actions, undefined)
  const previousFetch = globalThis.fetch
  const calls: { method: string; params: Record<string, unknown> }[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body))
    calls.push({ method: rpc.method, params: rpc.params })
    const result =
      rpc.method === "page_view"
        ? {
            type: "found",
            data: {
              page: { slug: "guild:welcome", created_at: "2025-01-01T00:00:00Z" },
              page_revision: { title: "Welcome", revision_number: 1 },
              options: { title: null },
              compiled_body_html: "<p>Welcome</p>",
              redirect_page: null
            }
          }
        : {}
    return Response.json({ jsonrpc: "2.0", id: rpc.id, result })
  }
  try {
    const parent = async () => ({
      site: { layout: "wikidot" },
      locales: ["fr", "en"],
      license_name: "CC BY-SA",
      license_url: "https://example.test/license"
    })
    const data = await printRoute.load({
      params: { path: "guild:welcome/extra" },
      request: new Request("https://wiki.example/printer--friendly/guild:welcome/extra", {
        headers: {
          "X-Wikijump-Site-Id": "6000011",
          "X-Wikijump-Site-Slug": "cobalt-company"
        }
      }),
      cookies: { get: () => "token-123" },
      parent,
      locals: {}
    })
    assert.equal(data.printView, true)
    assert.equal(data.sourceUrl, "https://wiki.example/guild:welcome")
    assert.equal(data.page_revision.title, "Welcome")
    assert.deepEqual(calls.find((call) => call.method === "page_view")?.params, {
      site_id: 6000011,
      locales: ["fr", "en"],
      session_token: "token-123",
      route: { slug: "guild:welcome", extra: "extra" }
    })
    assert.equal(
      calls.some((call) => call.method.startsWith("page_lock")),
      false
    )
  } finally {
    globalThis.fetch = previousFetch
  }
})

test("print root resolves default page through parent and denies missing or forbidden pages", async () => {
  const previousFetch = globalThis.fetch
  const routes: unknown[] = []
  let type = "found"
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body))
    if (rpc.method === "page_view") routes.push(rpc.params.route)
    const result =
      rpc.method === "page_view"
        ? {
            type,
            data: {
              page: { slug: "guild:welcome", created_at: "2025-01-01T00:00:00Z" },
              page_revision: { title: "Welcome", revision_number: 1 },
              options: { title: null },
              redirect_page: null
            }
          }
        : {}
    return Response.json({ jsonrpc: "2.0", id: rpc.id, result })
  }
  try {
    const event = {
      params: { path: "" },
      request: new Request("https://wiki.example/printer--friendly/", {
        headers: {
          "X-Wikijump-Site-Id": "6000011",
          "X-Wikijump-Site-Slug": "cobalt-company"
        }
      }),
      cookies: { get: () => undefined },
      parent: async () => ({
        site: { layout: "wikidot", default_page: "guild:welcome" },
        locales: ["en"],
        license_name: "CC",
        license_url: "https://example.test"
      }),
      locals: {}
    }
    const data = await printRoute.load(event)
    assert.equal(data.sourceUrl, "https://wiki.example/guild:welcome")
    assert.deepEqual(routes, [null])
    for (const [view, status] of [
      ["permissions", 403],
      ["missing", 404]
    ] as const) {
      type = view
      await assert.rejects(printRoute.load(event), (error: { status: number }) => {
        assert.equal(error.status, status)
        return true
      })
    }
  } finally {
    globalThis.fetch = previousFetch
  }
})
