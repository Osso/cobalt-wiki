import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { default: HistoryDetail } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/HistoryDetail.svelte"
)
const { render } = await vite.ssrLoadModule("svelte/server")

const revision = {
  id: 4,
  number: 9,
  source: "<script>bad</script>",
  rendered_html: null,
  representation: "display-decoded-not-byte-exact"
}

test("source stays read-only and retains captured Wikidot provenance", () => {
  const body = render(HistoryDetail, {
    props: { origin: "wikidot", revision, comparison: null, rendered: false }
  }).body
  assert.match(body, /readonly/)
  assert.match(body, /Captured Wikidot source; historical whitespace may differ/)
  assert.match(body, /display-decoded-not-byte-exact/)
  assert.doesNotMatch(body, /<script>/)
  assert.match(body, /&lt;script>/)
  assert.doesNotMatch(body, /rollback|<form/i)
})

test("diff escapes untrusted source lines and marks insertion/deletion", () => {
  const comparison = {
    from: 8,
    to: 9,
    representation: "captured",
    lines: [
      { kind: "delete", text: "<img src=x>" },
      { kind: "insert", text: "<script>" }
    ]
  }
  const body = render(HistoryDetail, {
    props: { origin: "wikidot", revision: null, comparison, rendered: false }
  }).body
  assert.match(body, /<del/)
  assert.match(body, /<ins/)
  assert.doesNotMatch(body, /<img|<script/)
  assert.match(body, /&lt;img/)
  assert.match(body, /&lt;script/)
})

test("rendered revision is labeled current context and remains inside panel", () => {
  const body = render(HistoryDetail, {
    props: {
      origin: "wikidot",
      revision: { ...revision, rendered_html: "<p>historic</p>" },
      comparison: null,
      rendered: true
    }
  }).body
  assert.match(body, /current rendering context/i)
  assert.match(body, /read-only preview/i)
  assert.match(body, /historic/)
  assert.doesNotMatch(body, /id="page-content"/)
})
