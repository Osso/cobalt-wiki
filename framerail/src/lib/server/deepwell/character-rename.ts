import { client } from "$lib/server/deepwell"
import type { RequestContext } from "$lib/server/load/request-ctx"

export interface CharacterRenamePage {
  page_id: number
  slug: string
  title: string
}

export interface CharacterRenamePreview {
  old_name: string
  new_name: string
  old_slug: string
  new_slug: string
  preview_token: string
  icon: boolean
  count: number
  pages: CharacterRenamePage[]
}

export interface CharacterRenameStatus {
  operation_id: number
  status: "queued" | "running" | "done" | "failed"
  done: number
  total: number
  error: string | null
  new_slug: string
}

export async function characterRenamePreview(
  pageId: number,
  newName: string,
  context: RequestContext
): Promise<CharacterRenamePreview> {
  return client.request(
    "character_rename_preview",
    { page_id: pageId, new_name: newName },
    context
  )
}

export async function characterRenameStart(
  pageId: number,
  newName: string,
  previewToken: string,
  ipAddress: string,
  context: RequestContext
): Promise<{ operation_id: number }> {
  return client.request(
    "character_rename_start",
    {
      page_id: pageId,
      new_name: newName,
      preview_token: previewToken,
      ip_address: ipAddress
    },
    context
  )
}

export async function characterRenameStatus(
  operationId: number,
  context: RequestContext
): Promise<CharacterRenameStatus> {
  return client.request("character_rename_status", { operation_id: operationId }, context)
}
