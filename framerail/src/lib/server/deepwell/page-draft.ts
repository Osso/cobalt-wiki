import type { FormValues } from "$lib/form-editor"
import { client } from "$lib/server/deepwell"
import type { RequestContext } from "$lib/server/load/request-ctx"

export interface PageDraft {
  title: string
  wikitext: string
  updated_at: string
  form_values?: FormValues
}

export interface PageDraftResponse {
  draft: PageDraft | null
}

type DraftContent =
  | { wikitext: string; form_updates?: never }
  | { wikitext?: never; form_updates: FormValues }

export type PageDraftRequest = {
  title: string
  last_revision_id?: number
} & DraftContent

export async function pageDraftGet(context: RequestContext): Promise<PageDraftResponse> {
  return client.request("page_draft_get", {}, context)
}

export async function pageDraftSave(
  payload: PageDraftRequest,
  context: RequestContext
): Promise<PageDraftResponse> {
  return client.request("page_draft_save", payload, context)
}

export async function pageDraftDelete(
  context: RequestContext
): Promise<{ deleted: boolean }> {
  return client.request("page_draft_delete", {}, context)
}
