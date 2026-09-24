import assert from "node:assert/strict"
import { test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
const { pageLayout } = await vite.ssrLoadModule("/src/lib/page-layout.ts")
await close()

const wikidotSite = { site: { layout: "wikidot" } }

test("wiki pages use the page's, then the site's layout", () => {
  assert.equal(
    pageLayout({ route: { id: "/[slug]/[...extra]" }, data: wikidotSite }),
    "wikidot"
  )
  assert.equal(
    pageLayout({
      route: { id: "/[slug]/[...extra]" },
      data: { ...wikidotSite, page: { layout: "wikijump" } }
    }),
    "wikijump"
  )
  assert.equal(pageLayout({ route: { id: null }, data: wikidotSite }), "wikidot")
})

test("account pages use the site's layout; other special pages use Wikijump's", () => {
  assert.equal(
    pageLayout({ route: { id: "/[x+2d]/login" }, data: wikidotSite }),
    "wikidot"
  )
  assert.equal(
    pageLayout({ route: { id: "/[x+2d]/settings" }, data: wikidotSite }),
    "wikidot"
  )
  for (const id of ["/[x+2d]/forgot-password", "/[x+2d]/set-password/[token]"]) {
    assert.equal(pageLayout({ route: { id }, data: wikidotSite }), "wikidot")
  }
  assert.equal(
    pageLayout({ route: { id: "/[x+2d]/admin" }, data: wikidotSite }),
    "wikijump"
  )
})
