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
    author_slug: null,
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
    author_slug: null,
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
        available: { wikidot: true, local: true },
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
  assert.doesNotMatch(body, /Page history of changes|<th[^>]*>compare<\/th>/)
  assert.match(body, /Wikidot ID 12/)
  assert.match(body, /Unknown/)
  assert.match(body, /View revision 9/)
  assert.match(body, /View source of revision 9/)
  assert.match(body, /name="from"/)
  assert.match(body, /name="to"/)
  assert.match(body, /Compare from revision 9/)
  assert.match(body, /Compare to revision 9/)
  assert.doesNotMatch(body, />from<\/label>|>to<\/label>/)
  assert.match(
    body,
    /<time datetime="2020-01-02T00:00:00Z"[^>]*>2 Jan 2020, 00:00<\/time>/
  )
  assert.match(body, /Date \(UTC\)/)
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
  assert.ok(body.indexOf('aria-label="History pages"') < body.indexOf("<table"))
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
      available: { wikidot: true, local: true },
      rows: [{ ...rows[1], author_name: "Ada" }]
    }
  })
  assert.match(body, /Ada/)
  assert.doesNotMatch(body, /Wikidot ID 12/)
})

test("flags render as named change chips with their source letter", () => {
  const body = display({
    listing: {
      origin: "wikidot",
      page: 1,
      per_page: 20,
      total: 1,
      total_pages: 1,
      available: { wikidot: true, local: false },
      rows: [{ ...rows[0], flags: ["N", "S", "A"], created_at: "2026-06-08T14:23:51Z" }]
    }
  })
  assert.match(body, /data-flag="N"[^>]*title="Page created"[^>]*>New</)
  assert.match(body, /data-flag="S"[^>]*title="Source changed"[^>]*>Source</)
  assert.match(body, /data-flag="A"[^>]*title="Tags changed"[^>]*>Tags</)
  assert.match(body, />8 Jun 2026, 14:23</)
  assert.match(body, /aria-label="View revision 9"[^>]*>View</)
  assert.match(body, /aria-label="View source of revision 9"[^>]*>Wikitext</)
  assert.match(body, />From<[\s\S]*>To</)
})

test("authors with a local account link to their user page; others stay plain text", () => {
  const body = display({
    listing: {
      origin: "wikidot",
      page: 1,
      per_page: 20,
      total: 2,
      total_pages: 1,
      available: { wikidot: true, local: true },
      rows: [
        {
          ...rows[0],
          author_id: 7444794,
          author_name: "OzmaAsimov",
          author_slug: "ozmaasimov"
        },
        { ...rows[1], author_id: 12, author_name: "Former Member", author_slug: null }
      ]
    }
  })
  assert.match(body, /<a href="\/-\/user\/ozmaasimov"[^>]*>OzmaAsimov<\/a>/)
  assert.match(body, />Former Member</)
  assert.doesNotMatch(body, /<a[^>]*>Former Member<\/a>/)
})
