import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { default: TagsPane } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/TagsPane.svelte"
)
const { default: Page } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/+page.svelte"
)
const { render } = await vite.ssrLoadModule("svelte/server")
const { pageLayoutState } = await vite.ssrLoadModule("/src/lib/stores.svelte.ts")
const { Layout } = await vite.ssrLoadModule("/src/lib/types.ts")

const data = {
  site: { name: "Cobalt" },
  page: { slug: "writing:example" },
  page_revision: { title: "Example", tags: ["original", "draft"] },
  options: {},
  compiled_body_html: "<p>Body</p>",
  internationalization: {
    edit: "Edit",
    vote: "Vote",
    tags: "Tags",
    history: "History",
    files: "Files",
    options: "More options",
    close: "Close",
    cancel: "Cancel",
    save: "Save"
  }
}

function renderTags(layout: string, tags = data.page_revision.tags) {
  pageLayoutState.current = layout
  return render(TagsPane, {
    props: {
      data: { ...data, page_revision: { ...data.page_revision, tags } },
      close() {},
      saveTagChanges() {}
    }
  }).body
}

test("Wikidot Tags pane renders source form and all three controls", () => {
  const body = renderTags(Layout.WIKIDOT)
  assert.match(body, /<h1[^>]*>Page Tags<\/h1>/)
  assert.match(body, /Tags are a nice way to organize content/)
  assert.match(body, /href="http:\/\/en.wikipedia.org\/wiki\/Tags"/)
  assert.match(body, /href="http:\/\/en.wikipedia.org\/wiki\/Tag_cloud"/)
  assert.match(body, /<table class="form(?: [^"]+)?">/)
  assert.match(
    body,
    /<input[^>]*class="text(?: [^"]+)?"[^>]*size="50"[^>]*value="draft original"/
  )
  assert.match(body, /Space-separated list of tags/)
  assert.match(body, /value="close"/)
  assert.match(body, /value="clear"/)
  assert.match(body, /value="save tags"/)
})

test("Wikidot Tags input sorts stored tags without changing alternate layout", () => {
  const tags = ["zulu", "alpha", "middle"]
  assert.match(renderTags(Layout.WIKIDOT, tags), /value="alpha middle zulu"/)
  assert.match(renderTags(Layout.WIKIJUMP, tags), /value="zulu alpha middle"/)
})

test("alternate Tags layout retains its existing controls", () => {
  const body = renderTags(Layout.WIKIJUMP)
  assert.match(body, /value="Cancel"/)
  assert.match(body, /value="Save"/)
  assert.doesNotMatch(body, /Page Tags/)
})

test("Wikidot page omits Vote while keeping its other bottom actions", () => {
  pageLayoutState.current = Layout.WIKIDOT
  const body = render(Page, { props: { data } }).body
  assert.doesNotMatch(body, /id="pagerate-button"/)
  for (const id of [
    "edit-button",
    "tags-button",
    "history-button",
    "files-button",
    "more-options-button"
  ]) {
    assert.match(body, new RegExp(`id="${id}"`))
  }
})

test("alternate page layout retains Vote", () => {
  pageLayoutState.current = Layout.WIKIJUMP
  assert.match(render(Page, { props: { data } }).body, /button-vote/)
})
