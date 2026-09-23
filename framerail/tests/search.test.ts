import assert from "node:assert/strict"
import { after, test } from "node:test"
import { readFile, unlink, writeFile } from "node:fs/promises"
import { compile, preprocess } from "svelte/compiler"
import { render } from "svelte/server"
import ts from "typescript"
import { createServer } from "vite"

const vite = await createServer({ server: { middlewareMode: true }, logLevel: "error" })
after(() => vite.close())
const { load } = await vite.ssrLoadModule("/src/routes/search:site/+page.server.ts")
async function loadComponent(path: string, name: string) {
  const source = await readFile(new URL(path, import.meta.url), "utf8")
  const prepared = await preprocess(source, {
    script: ({ content, attributes }) =>
      attributes.lang === "ts"
        ? {
            code: ts.transpileModule(content, {
              compilerOptions: { target: ts.ScriptTarget.ESNext }
            }).outputText
          }
        : undefined
  })
  const compiled = compile(prepared.code, { generate: "server", filename: name })
  const fixture = new URL(`./.search-${name}-${process.pid}.mjs`, import.meta.url)
  await writeFile(fixture, compiled.js.code)
  after(() => unlink(fixture))
  return (await import(fixture.href)).default
}

const SearchPage = await loadComponent("../src/routes/search:site/+page.svelte", "page")
const SearchBox = await loadComponent("../src/lib/component/SearchBox.svelte", "box")

const hit = (index: number) => ({
  page_id: index,
  title: index === 1 ? '<img src=x onerror="alert(1)">' : `Page ${index}`,
  slug: `page-${index}`,
  tags: ["reference"],
  snippet: index === 1 ? "<script>alert(1)</script>" : `Summary ${index}`
})

type Rpc = { method: string; params: Record<string, unknown>; id: string | number }
type RpcResponse =
  | { result: { hits: ReturnType<typeof hit>[]; has_more: boolean } }
  | { error: { code: number; message: string } }

async function search(
  url: string,
  responder: (rpc: Rpc) => RpcResponse = () => ({
    result: { hits: [], has_more: false }
  })
) {
  const previousFetch = globalThis.fetch
  const calls: { rpc: Rpc; headers: Headers }[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body)) as Rpc
    calls.push({ rpc, headers: new Headers(init?.headers) })
    const response = responder(rpc)
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, ...response }))
  }
  const locals: {
    requestContext: { siteId: number; sessionToken: string }
    documentLayout?: string
  } = {
    requestContext: { siteId: 6000011, sessionToken: "session-secret" }
  }
  try {
    const data = await load({
      url: new URL(url),
      locals,
      parent: async () => ({ site: { layout: "wikidot" } })
    } as never)
    return { data, calls, locals }
  } finally {
    globalThis.fetch = previousFetch
  }
}

test("GET form uses source theme selectors and submits a named query to the search route", () => {
  const { body } = render(SearchBox)
  assert.match(body, /id="search-top-box"/)
  assert.match(body, /<form[^>]*method="GET"[^>]*action="\/search:site"/)
  assert.match(body, /id="search-top-box-input"[^>]*name="query"/)
  assert.match(body, /<label[^>]*for="search-top-box-input"/)
  assert.match(body, /<button[^>]*type="submit"[^>]*>Search<\/button>/)
})

test("search sends only bounded query parameters with trusted site and session headers", async () => {
  const { data, calls, locals } = await search(
    "http://local.test/search:site?query=knowledge&offset=0&site_id=9&user_id=9",
    () => ({ result: { hits: [hit(1)], has_more: true } })
  )
  assert.equal(locals.documentLayout, "wikidot")
  assert.equal(calls.length, 1)
  assert.equal(calls[0].rpc.method, "page_search")
  assert.deepEqual(calls[0].rpc.params, { query: "knowledge", offset: 0, limit: 20 })
  assert.equal(calls[0].headers.get("X-Deepwell-Site-Id"), "6000011")
  assert.equal(calls[0].headers.get("X-Deepwell-Session-Token"), "session-secret")
  assert.equal(calls[0].headers.get("X-Deepwell-Page"), null)
  assert.equal(data.hits.length, 1)
})

test("two pages past 20 hits preserve query and provide previous/next navigation", async () => {
  const query = "tag:reference & <articles>"
  const base = `http://local.test/search:site?query=${encodeURIComponent(query)}`
  const first = await search(base, () => ({
    result: {
      hits: Array.from({ length: 20 }, (_, index) => hit(index + 1)),
      has_more: true
    }
  }))
  const firstHtml = render(SearchPage, { props: { data: first.data } }).body
  assert.match(firstHtml, /page-20/)
  assert.match(firstHtml, /offset=20/)
  assert.match(firstHtml, /query=tag%3Areference/)
  assert.doesNotMatch(firstHtml, /<script>alert\(1\)<\/script>/)
  assert.doesNotMatch(firstHtml, /<img src=x onerror=/)
  assert.match(firstHtml, /&lt;script>alert\(1\)&lt;\/script>/)

  const second = await search(`${base}&offset=20`, () => ({
    result: {
      hits: Array.from({ length: 5 }, (_, index) => hit(index + 21)),
      has_more: false
    }
  }))
  assert.deepEqual(second.calls[0].rpc.params, { query, offset: 20, limit: 20 })
  const secondHtml = render(SearchPage, { props: { data: second.data } }).body
  assert.match(secondHtml, /page-25/)
  assert.match(secondHtml, /offset=0/)
  assert.doesNotMatch(secondHtml, /offset=40/)
})

test("empty queries skip RPC; invalid and excessive offsets are bounded", async () => {
  const blank = await search("http://local.test/search:site?query=%20%20")
  assert.equal(blank.calls.length, 0)
  assert.match(
    render(SearchPage, { props: { data: blank.data } }).body,
    /Enter a search query/
  )

  for (const offset of [
    "-1",
    "3.2",
    "Infinity",
    "501",
    "999999999999999999999999999999"
  ]) {
    const result = await search(
      `http://local.test/search:site?query=test&offset=${offset}`
    )
    assert.equal(result.calls[0].rpc.params.offset, 0)
  }
})

test("empty result and unavailable search show distinct clear messages", async () => {
  const empty = await search("http://local.test/search:site?query=absent", () => ({
    result: { hits: [], has_more: false }
  }))
  assert.match(
    render(SearchPage, { props: { data: empty.data } }).body,
    /No results found/
  )
  const unavailable = await search("http://local.test/search:site?query=absent", () => ({
    error: { code: -32603, message: "index offline" }
  }))
  assert.match(
    render(SearchPage, { props: { data: unavailable.data } }).body,
    /Search is temporarily unavailable/
  )
})
