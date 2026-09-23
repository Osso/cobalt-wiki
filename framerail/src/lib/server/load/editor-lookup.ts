import { fail, type RequestEvent } from "@sveltejs/kit"
import { client } from "$lib/server/deepwell"
import { pageSearch } from "$lib/server/deepwell/search"
import { getRequestContext } from "./request-ctx"

export async function editorPagesAction({ request, locals }: RequestEvent) {
  const form = await request.formData()
  const query = form.get("query")
  if (typeof query !== "string" || query.trim().length < 2) {
    return fail(400, { message: "Enter at least two characters" })
  }
  try {
    const result = await pageSearch(query.trim(), 0, 20, getRequestContext(locals))
    return { pages: result.hits.map(({ slug, title }) => ({ slug, title })) }
  } catch {
    return fail(500, { message: "Unable to look up pages" })
  }
}

export async function editorAttachmentsAction({ locals }: RequestEvent) {
  try {
    const files = await client.request<{ name: string }[]>(
      "editor_attachments",
      {},
      getRequestContext(locals)
    )
    return { files }
  } catch {
    return fail(403, { message: "Unable to load attached files" })
  }
}
