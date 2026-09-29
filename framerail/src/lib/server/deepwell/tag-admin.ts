import { client } from "$lib/server/deepwell"
import type { RequestContext } from "$lib/server/load/request-ctx"

export interface TagChange {
  source_tag: string
  target_tag: string | null
}

export interface TagPage {
  page_id: number
  slug: string
  title: string
}

export interface TagPreview extends TagChange {
  preview_token: string
  count: number
  pages: TagPage[]
}

export interface TagAddFilters {
  existing_tag: string | null
  category: string | null
  title_contains: string | null
}

export interface TagAddSelection extends TagAddFilters {
  tag: string
}

export interface TagAddPreview extends TagAddSelection {
  preview_token: string
  count: number
  pages: TagPage[]
}

export interface TagOperation {
  kind: "rename" | "delete" | "add"
  source_tag: string | null
  target_tag: string | null
  filters: TagAddFilters | null
  operation_id: number
  actor_user_id: number
  created_at: string
  count: number
  reversed: boolean
  pages: TagPage[]
}

export interface TagResult {
  operation_id: number
  count: number
}

export async function tagHistory(context: RequestContext): Promise<TagOperation[]> {
  return client.request("site_tag_history", {}, context)
}

export async function tagPreview(
  change: TagChange,
  context: RequestContext
): Promise<TagPreview> {
  return client.request("site_tag_preview", change, context)
}

export async function tagApply(
  change: TagChange,
  previewToken: string,
  ipAddress: string,
  context: RequestContext
): Promise<TagResult> {
  return client.request(
    "site_tag_apply",
    { ...change, preview_token: previewToken, ip_address: ipAddress },
    context
  )
}

export async function tagAddPreview(
  selection: TagAddSelection,
  context: RequestContext
): Promise<TagAddPreview> {
  return client.request("site_tag_add_preview", selection, context)
}

export async function tagAddApply(
  selection: TagAddSelection,
  previewToken: string,
  ipAddress: string,
  context: RequestContext
): Promise<TagResult> {
  return client.request(
    "site_tag_add_apply",
    { ...selection, preview_token: previewToken, ip_address: ipAddress },
    context
  )
}

export async function tagUndo(
  operationId: number,
  ipAddress: string,
  context: RequestContext
): Promise<TagResult> {
  return client.request(
    "site_tag_undo",
    { operation_id: operationId, ip_address: ipAddress },
    context
  )
}
