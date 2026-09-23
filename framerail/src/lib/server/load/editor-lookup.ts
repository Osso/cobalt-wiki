import { fail, type RequestEvent } from "@sveltejs/kit"
import { client } from "$lib/server/deepwell"
import { getRequestContext } from "./request-ctx"

export async function editorPagesAction({ request, locals }: RequestEvent) {
  const form = await request.formData()
  const query = form.get("query")
  if (typeof query !== "string" || query.trim().length < 2) {
    return fail(400, { message: "Enter at least two characters" })
  }
  try {
    const pages: { slug: string; title: string }[] = await client.request(
      "editor_pages",
      { query },
      getRequestContext(locals)
    )
    return { pages }
  } catch {
    return fail(500, { message: "Unable to look up pages" })
  }
}

export async function editorAttachmentsAction({ locals }: RequestEvent) {
  try {
    const files: { name: string }[] = await client.request(
      "editor_attachments",
      {},
      getRequestContext(locals)
    )
    return { files }
  } catch {
    return fail(403, { message: "Unable to load attached files" })
  }
}
