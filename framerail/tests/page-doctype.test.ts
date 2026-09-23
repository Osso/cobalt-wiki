import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

import type { ResolveOptions } from "@sveltejs/kit"

const { vite, close } = await createSsrServer()
after(close)
const { handle } = await vite.ssrLoadModule("/src/hooks.server.ts")
const root = await vite.ssrLoadModule("/src/routes/+page.server.ts")
const slug = await vite.ssrLoadModule("/src/routes/[slug]/[...extra]/+page.server.ts")

const html5 =
  '<!doctype html><html><body><span class="caption">Caption</span></body></html>'
const xhtml =
  '<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN" "http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd">'

async function pageResponse(
  siteLayout: string,
  pageLayout: string | null,
  path = "/who-we-are",
  viewType = "found"
) {
  const previousFetch = globalThis.fetch
  const calls: string[] = []
  globalThis.fetch = async (_input, init) => {
    const rpc = JSON.parse(String(init?.body))
    calls.push(rpc.method)
    const result =
      rpc.method === "page_view"
        ? {
            type: viewType,
            data:
              viewType === "found"
                ? {
                    page: {
                      layout: pageLayout,
                      created_at: "2024-01-01T00:00:00Z",
                      slug: "who-we-are"
                    },
                    page_revision: { revision_number: 1 },
                    redirect_page: null
                  }
                : { redirect_page: null }
          }
        : {}
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, result }))
  }
  const request = new Request(`http://local.test${path}`, {
    headers: { "X-Wikijump-Site-Id": "6000011", "X-Wikijump-Site-Slug": "cobalt-company" }
  })
  const locals = {}
  const event = {
    request,
    cookies: { get: () => undefined },
    locals,
    params: path === "/" ? {} : { slug: "who-we-are", extra: undefined }
  }
  const parent = async () => ({
    site: { layout: siteLayout },
    locales: ["en"],
    license_name: "CC",
    license_url: "https://example.test"
  })
  try {
    const response = await handle({
      event,
      resolve: async (handledEvent: typeof event, options?: ResolveOptions) => {
        const route = path === "/" ? root : slug
        try {
          await route.load({ ...handledEvent, parent })
        } catch (error) {
          if (viewType !== "missing") throw error
          // The error page still receives the HTML5 document template.
        }
        const chunks: string[] = []
        const documentChunks = [html5.slice(0, 8), html5.slice(8)]
        for (const [index, chunk] of documentChunks.entries()) {
          chunks.push(
            (await options?.transformPageChunk?.({
              html: chunk,
              done: index === documentChunks.length - 1
            })) ?? chunk
          )
        }
        return new Response(chunks.join(""), { headers: { "content-type": "text/html" } })
      }
    })
    return { html: await response.text(), calls }
  } finally {
    globalThis.fetch = previousFetch
  }
}

test("Wikidot site page emits the original XHTML doctype without another backend request", async () => {
  const { html, calls } = await pageResponse("wikidot", null)
  assert.equal(html, xhtml + html5.slice("<!doctype html>".length))
  assert.deepEqual(calls, ["page_view", "translate"])
})

test("page override controls doctype independently of site layout", async () => {
  assert.equal((await pageResponse("wikijump", "wikidot")).html.startsWith(xhtml), true)
  assert.equal((await pageResponse("wikidot", "wikijump")).html, html5)
})

test("failed Wikidot page load leaves the error document HTML5", async () => {
  assert.equal(
    (await pageResponse("wikidot", null, "/who-we-are", "missing")).html,
    html5
  )
})

test("root Wikidot page emits XHTML; native page and special responses retain HTML5", async () => {
  assert.equal((await pageResponse("wikidot", null, "/")).html.startsWith(xhtml), true)
  assert.equal((await pageResponse("wikijump", null)).html, html5)
  const request = new Request("http://local.test/-/admin", {
    headers: { "X-Wikijump-Site-Id": "6000011", "X-Wikijump-Site-Slug": "cobalt-company" }
  })
  const response = await handle({
    event: { request, cookies: { get: () => undefined }, locals: {}, params: {} },
    resolve: async (_event: unknown, options?: ResolveOptions) =>
      new Response(
        (await options?.transformPageChunk?.({ html: html5, done: true })) ?? html5,
        { headers: { "content-type": "text/html" } }
      )
  })
  assert.equal(await response.text(), html5)
})
