import assert from "node:assert/strict"
import { after, test } from "node:test"
import { createSsrServer } from "./ssr-server.ts"

const { vite, close } = await createSsrServer()
after(close)
const { default: FilePane } = await vite.ssrLoadModule(
  "/src/routes/[slug]/[...extra]/FilePane.svelte"
)
const { render } = await vite.ssrLoadModule("svelte/server")
const { setContext } = await vite.ssrLoadModule("svelte")
const { readable } = await vite.ssrLoadModule("svelte/store")
const { pageLayoutState } = await vite.ssrLoadModule("/src/lib/stores.svelte.ts")
const { Layout } = await vite.ssrLoadModule("/src/lib/types.ts")

const files = [
  {
    file_id: 12,
    file_created_at: "2024-03-01T12:30:00Z",
    file_updated_at: null,
    file_deleted_at: null,
    page_id: 42,
    revision_id: 72,
    revision_type: "create",
    revision_created_at: "2024-03-01T12:30:00Z",
    revision_number: 1,
    revision_user_id: 101,
    name: "report.pdf",
    data: null,
    mime: "application/pdf",
    size: 1025,
    s3_hash: "abc",
    revision_comments: "First report",
    hidden_fields: []
  },
  {
    file_id: 13,
    file_created_at: "2024-03-02T11:00:00Z",
    file_updated_at: null,
    file_deleted_at: null,
    page_id: 42,
    revision_id: 73,
    revision_type: "regular",
    revision_created_at: "2024-03-02T11:00:00Z",
    revision_number: 2,
    revision_user_id: 102,
    name: "map.png",
    data: null,
    mime: "image/png",
    size: 3072,
    s3_hash: "def",
    revision_comments: "",
    hidden_fields: []
  },
  {
    file_id: 14,
    file_created_at: "2024-03-03T11:00:00Z",
    file_updated_at: "2024-03-04T11:00:00Z",
    file_deleted_at: "2024-03-04T11:00:00Z",
    page_id: 42,
    revision_id: 74,
    revision_type: "delete",
    revision_created_at: "2024-03-04T11:00:00Z",
    revision_number: 2,
    revision_user_id: 103,
    name: "old.txt",
    data: null,
    mime: "text/plain",
    size: 9999,
    s3_hash: "ghi",
    revision_comments: "Removed",
    hidden_fields: []
  }
]

const data = {
  site: { site_id: 1 },
  site_file_domain: "files.example.test",
  page: { page_id: 42, slug: "writing:example" },
  forms: Object.fromEntries(
    ["fileUploadForm", "fileEditForm", "fileMoveForm", "fileRestoreForm"].map((name) => [
      name,
      { valid: false, posted: false, errors: {}, data: {} }
    ])
  ),
  internationalization: {
    "wiki-page-file": "Files",
    "wiki-page-file.name": "Name",
    "wiki-page-file.size": "Size",
    "wiki-page-file-no-files": "No files attached to this page.",
    restore: "Restore",
    history: "History",
    move: "Move",
    edit: "Edit",
    delete: "Delete"
  }
}

function renderFiles(layout: string, listedFiles = files) {
  pageLayoutState.current = layout
  function PageContext(payload: unknown, props: unknown) {
    setContext("__svelte__", {
      page: readable({ url: new URL("https://example.test/writing:example") }),
      navigating: readable(null),
      updated: { subscribe: readable(false).subscribe }
    })
    FilePane(payload, props)
  }
  return render(PageContext, { props: { data, initialFiles: listedFiles } }).body
}

test("Wikidot Files shows total bytes of listed nondeleted files only", () => {
  const body = renderFiles(Layout.WIKIDOT)
  assert.match(body, /Total files size:\s*4,097 Bytes/)
  assert.doesNotMatch(body, /14,096 Bytes/)
  assert.doesNotMatch(renderFiles(Layout.WIKIDOT, []), /Total files size:/)
  assert.match(
    renderFiles(Layout.WIKIDOT, [{ ...files[0], size: 0 }]),
    /Total files size:\s*0 Bytes/
  )
})

test("Wikidot Files supplies read-only information for an active file", () => {
  const body = renderFiles(Layout.WIKIDOT)
  assert.match(body, /<details[^>]*>[\s\S]*?<summary[^>]*>info<\/summary>/)
  assert.match(body, /File Information/)
  assert.match(body, /Full file URL/)
  assert.match(
    body,
    /href="\/\/files\.example\.test\/-\/file\/writing:example\/report\.pdf"/
  )
  assert.match(body, /application\/pdf/)
  assert.match(body, /1,025 Bytes/)
  assert.match(body, /First report/)
  assert.match(body, /2024/)
  assert.doesNotMatch(body, /Uploaded by|upload limit/)
  assert.equal((body.match(/<summary[^>]*>info<\/summary>/g) ?? []).length, 2)
})

test("alternate Files layout retains existing presentation without Wikidot additions", () => {
  const body = renderFiles(Layout.WIKIJUMP)
  assert.doesNotMatch(
    body,
    /Total files size:|File Information|<summary[^>]*>info<\/summary>/
  )
  assert.match(body, /file-history clickable/)
  assert.match(body, /application\/pdf/)
})
