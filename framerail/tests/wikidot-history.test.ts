import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { default: WikidotHistory } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/WikidotHistory.svelte"
)
const { default: HistoryPane } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/HistoryPane.svelte"
)
const { pageLayoutState } = await vite.ssrLoadModule("/src/lib/stores.svelte.ts")
const { Layout } = await vite.ssrLoadModule("/src/lib/types.ts")
const { render } = await vite.ssrLoadModule("svelte/server")

test("Wikidot pane replaces stacked imported and legacy history tables", () => {
  pageLayoutState.current = Layout.WIKIDOT
  const body = render(HistoryPane, {
    props: {
      data: {
        site: { site_id: 6000011 },
        page: { page_id: 42 },
        internationalization: {}
      },
      setRevision() {},
      setShowRevision() {}
    }
  }).body
  assert.match(body, /Show page changes/)
  assert.doesNotMatch(body, /Imported Wikidot history|Load older revisions/)
  assert.equal((body.match(/page-revision-header/g) ?? []).length, 0)
})

test("Wikidot history presents one dataset selector and original filter/page-size controls", () => {
  const body = render(WikidotHistory).body
  assert.match(body, /Show page changes/)
  for (const label of [
    "ALL",
    "source changes",
    "title",
    "move\/rename",
    "meta data",
    "attachments"
  ])
    assert.match(body, new RegExp(label))
  for (const size of [10, 20, 50, 100, 200])
    assert.match(body, new RegExp(`value="${size}"`))
  assert.match(body, /value="20"[^>]*selected/)
  assert.match(body, /update list/)
  assert.match(body, /compare versions/)
  assert.match(body, /History dataset/)
  assert.doesNotMatch(body, /Imported Wikidot history|Load older revisions/)
})
