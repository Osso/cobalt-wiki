import { client } from "$lib/server/deepwell"
import type { RequestContext } from "$lib/server/load/request-ctx"

export interface SearchHit {
  page_id: number
  title: string
  slug: string
  tags: string[]
  snippet: string
}

export interface SearchPage {
  hits: SearchHit[]
  has_more: boolean
}

export async function pageSearch(
  query: string,
  offset: number,
  limit: number,
  requestContext: RequestContext
): Promise<SearchPage> {
  return client.request("page_search", { query, offset, limit }, requestContext)
}
