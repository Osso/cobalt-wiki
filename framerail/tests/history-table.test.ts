import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { default: HistoryTable } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/HistoryTable.svelte"
)
const { render } = await vite.ssrLoadModule("svelte/server")

const rows = [
  {
    id: 44,
    number: 9,
    flags: ["S"],
    author_id: null,
    author_name: null,
    created_at: "2020-01-02T00:00:00Z",
    comments: "<script>alert(1)</script>",
    is_current: true,
    representation: "captured"
  },
  {
    id: 43,
    number: 8,
    flags: ["T"],
    author_id: 12,
    author_name: null,
    created_at: "2020-01-01T00:00:00Z",
    comments: "previous",
    is_current: false,
    representation: "captured"
  }
]
function display(overrides: Record<string, unknown> = {}) {
  return render(HistoryTable, {
    props: {
      listing: {
        origin: "wikidot",
        page: 2,
        per_page: 20,
        total: 41,
        total_pages: 3,
        available: { wikidot: 41, local: 3 },
        rows
      },
      from: 8,
      to: 9,
      busy: false,
      selectFrom: () => {},
      selectTo: () => {},
      changePage: () => {},
      openRevision: () => {},
      ...overrides
    }
  }).body
}

test("one table labels revision actions and unknown Wikidot authors without invented metadata", () => {
  const body = display()
  assert.match(body, /Page history of changes/)
  assert.match(body, /Wikidot ID 12/)
  assert.match(body, /Unknown/)
  assert.match(body, /View revision 9/)
  assert.match(body, /View source of revision 9/)
  assert.match(body, /name="from"/)
  assert.match(body, /name="to"/)
  assert.doesNotMatch(body, /<script>/)
  assert.match(body, /&lt;script>alert\(1\)&lt;\/script>/)
  assert.doesNotMatch(body, /rollback|avatar/i)
})

test("numbered pager exposes all pages and total, including page two", () => {
  const body = display()
  assert.match(body, /41 revisions/)
  assert.match(body, /Page 2 of 3/)
  assert.match(body, /Go to page 1/)
  assert.match(body, /Go to page 2/)
  assert.match(body, /Go to page 3/)
  assert.match(body, /aria-current="page"/)
})

test("local dataset keeps local provenance rather than Wikidot author IDs", () => {
  const body = display({
    listing: {
      origin: "local",
      page: 1,
      per_page: 20,
      total: 1,
      total_pages: 1,
      available: { wikidot: 2, local: 1 },
      rows: [{ ...rows[1], author_name: "Ada" }]
    }
  })
  assert.match(body, /Ada/)
  assert.doesNotMatch(body, /Wikidot ID 12/)
})
