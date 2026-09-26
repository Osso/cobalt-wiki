import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const {
  default: ParentPane,
  parentLookupQuery,
  parentSuggestionValue,
  fetchParentSuggestions
} = await vite.ssrLoadModule("/src/routes/[slug]/[...extra]/ParentPane.svelte")
const { render } = await vite.ssrLoadModule("svelte/server")
const { readable } = await vite.ssrLoadModule("svelte/store")
const { pageLayoutState } = await vite.ssrLoadModule("/src/lib/stores.svelte.ts")
const { Layout, PagePane } = await vite.ssrLoadModule("/src/lib/types.ts")

function renderParent(layout: string) {
  pageLayoutState.current = layout
  return render(ParentPane, {
    props: {
      pagePaneState: PagePane.Parent,
      data: {
        site: { site_id: 1 },
        page: { page_id: 2, slug: "child" },
        forms: {
          pageParentForm: {
            id: "parent",
            valid: false,
            posted: false,
            errors: {},
            data: { siteId: 1, pageId: 2, parents: "" },
            constraints: {}
          }
        },
        internationalization: {
          "wiki-page-parent": "Parent page",
          parents: "Parents",
          cancel: "Cancel",
          save: "Save"
        }
      }
    },
    context: new Map([
      [
        "__svelte__",
        {
          page: readable({ url: new URL("http://local.test/child"), form: null }),
          navigating: readable(null),
          updated: readable(false)
        }
      ]
    ])
  }).body
}

test("Wikidot parent pane describes plural breadcrumb parents with labeled lookup and clear", () => {
  const body = renderParent(Layout.WIKIDOT)
  assert.match(body, /breadcrumbs/i)
  assert.match(body, /space-separated/i)
  assert.match(body, /<label[^>]*for="parent-page-names"[^>]*>Parent page names<\/label>/)
  assert.match(
    body,
    /<input[^>]*id="parent-page-names"[^>]*list="parent-page-suggestions"/
  )
  assert.match(body, /<datalist id="parent-page-suggestions"/)
  assert.match(body, /Clear parents/)
  assert.match(body, /value="Cancel"/)
  assert.match(body, /value="Save"/)
})

test("alternate parent layout retains controls and plural form", () => {
  const body = renderParent(Layout.WIKIJUMP)
  assert.match(body, /page-parent-new-parents/)
  assert.match(body, /Cancel/)
  assert.match(body, /Save/)
})

test("lookup targets only the last typed slug with at least two characters", () => {
  assert.equal(parentLookupQuery("alpha be"), "be")
  assert.equal(parentLookupQuery("alpha b"), "")
  assert.equal(parentLookupQuery("alpha "), "")
  assert.equal(parentLookupQuery("alpha  beta"), "beta")
})

test("selecting a suggestion retains every previously entered parent", () => {
  assert.equal(parentSuggestionValue("alpha  beta ga", "gamma"), "alpha  beta gamma")
  assert.equal(parentSuggestionValue("ga", "gamma"), "gamma")
})

test("late lookup response cannot replace suggestions after input changes", async () => {
  let resolveLookup!: (pages: { slug: string; title: string }[]) => void
  const lookup = () =>
    new Promise<{ slug: string; title: string }[]>((resolve) => {
      resolveLookup = resolve
    })
  let current = true
  const pending = fetchParentSuggestions("be", lookup, () => current)
  current = false
  resolveLookup([{ slug: "beta", title: "Beta" }])
  assert.equal(await pending, undefined)
})

test("lookup failure is visible for current input, ignored for stale input", async () => {
  const lookup = async () => {
    throw new Error("private detail")
  }
  assert.deepEqual(await fetchParentSuggestions("be", lookup, () => true), {
    matches: [],
    error: "Unable to look up pages. You can still enter page names."
  })
  assert.equal(await fetchParentSuggestions("be", lookup, () => false), undefined)
})
