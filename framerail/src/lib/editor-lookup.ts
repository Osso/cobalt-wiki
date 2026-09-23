import { deserialize } from "$app/forms"

async function lookup(action: string, body = new FormData()) {
  const response = await fetch(`?/${action}`, {
    method: "POST",
    headers: { accept: "application/json", "x-sveltekit-action": "true" },
    body
  })
  const result = deserialize(await response.text())
  if (result.type !== "success" || !result.data) {
    throw new Error("Editor lookup failed")
  }
  return result.data
}

export async function lookupEditorPages(
  query: string
): Promise<{ slug: string; title: string }[]> {
  const body = new FormData()
  body.set("query", query)
  const result = await lookup("editorPages", body)
  return result.pages as { slug: string; title: string }[]
}

export async function lookupEditorAttachments(
  slug: string
): Promise<{ name: string; url: string }[]> {
  const result = await lookup("editorAttachments")
  return (result.files as { name: string }[]).map(({ name }) => ({
    name,
    url: `/local--files/${encodeURIComponent(slug)}/${encodeURIComponent(name)}`
  }))
}
