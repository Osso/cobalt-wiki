import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { default: MoveDependencies } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/MoveDependencies.svelte"
)
const { default: MovePane } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/MovePane.svelte"
)
const { render } = await vite.ssrLoadModule("svelte/server")
const { readable } = await vite.ssrLoadModule("svelte/store")
const { pageLayoutState } = await vite.ssrLoadModule("/src/lib/stores.svelte.ts")
const { Layout, PagePane } = await vite.ssrLoadModule("/src/lib/types.ts")

const dependencies = {
  links: [
    { page_id: 12, slug: "notes:one & two", title: "One <script>" },
    { page_id: 13, slug: "notes:both", title: "Both" }
  ],
  inclusions: [
    { page_id: 13, slug: "notes:both", title: "Both" },
    { page_id: 14, slug: "notes:included", title: "Included" }
  ]
}

function renderDependencies(remaining = false, selectedIds: number[] = []) {
  return render(MoveDependencies, {
    props: { dependencies, selectedIds, remaining }
  }).body
}

test("move starts with an on-demand list button and no repair selection", () => {
  pageLayoutState.current = Layout.WIKIDOT
  const html = render(MovePane, {
    props: {
      pagePaneState: PagePane.Move,
      data: {
        site: { site_id: 6000011 },
        page: { page_id: 42, slug: "home:start" },
        page_revision: { revision_id: 21 },
        forms: {
          pageMoveForm: {
            id: "move",
            valid: false,
            posted: false,
            errors: {},
            data: { siteId: 6000011, pageId: 42, newSlug: "home:new", comments: "" },
            constraints: {}
          }
        },
        internationalization: {
          "wiki-page-move": "Move page",
          "wiki-page-move.new-slug": "New name",
          move: "Move",
          cancel: "Cancel"
        }
      }
    },
    context: new Map([
      [
        "__svelte__",
        {
          page: readable({ url: new URL("http://local.test/home:start"), form: null }),
          navigating: readable(null),
          updated: readable(false)
        }
      ]
    ])
  }).body
  assert.match(html, /Show dependencies/)
  assert.doesNotMatch(html, /type="checkbox"/)
  assert.match(html, /value="home:new"/)
})

test("choices render one checkbox per numeric source ID, none selected by default", () => {
  const html = renderDependencies()
  assert.equal((html.match(/type="checkbox"/g) ?? []).length, 3)
  assert.match(html, /value="12"/)
  assert.match(html, /value="13"/)
  assert.match(html, /value="14"/)
  assert.doesNotMatch(html, /checked(?:\s|>|=)/)
  assert.match(html, /Select all/)
  assert.match(html, /Unselect all/)
  assert.match(html, /Edit/)
  assert.match(html, /href="\/notes%3Aone%20%26%20two"/)
  assert.doesNotMatch(html, /<script>/)
})

test("selected source renders checked without selecting other sources", () => {
  const html = renderDependencies(false, [13])
  assert.match(html, /value="13"[^>]*checked/)
  assert.doesNotMatch(html, /value="12"[^>]*checked/)
})

test("leftover result separates links and inclusions, with no repair controls", () => {
  const html = renderDependencies(true)
  assert.match(html, /<h[23][^>]*>Links<\/h[23]>/)
  assert.match(html, /<h[23][^>]*>Inclusions<\/h[23]>/)
  assert.equal((html.match(/notes%3Aboth/g) ?? []).length, 2)
  assert.doesNotMatch(html, /type="checkbox"|Select all/)
  assert.match(html, /manually edit/i)
})
