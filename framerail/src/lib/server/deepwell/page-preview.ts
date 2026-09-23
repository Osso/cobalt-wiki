import { client } from "$lib/server/deepwell"
import type { FormValues } from "$lib/form-editor"
import type { RequestContext } from "$lib/server/load/request-ctx"

type PreviewContent =
  | { wikitext: string; form_updates?: never }
  | {
      wikitext?: never
      form_updates: FormValues
    }

export type PagePreviewRequest = {
  title?: string
  alt_title?: string | null
  tags?: string[]
  last_revision_id?: number
} & PreviewContent

export interface PagePreviewResponse {
  html: string
}

export async function pagePreview(
  payload: PagePreviewRequest,
  requestContext: RequestContext
): Promise<PagePreviewResponse> {
  return client.request("page_preview", payload, requestContext)
}
