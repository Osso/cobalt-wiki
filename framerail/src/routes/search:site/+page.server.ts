import { pageSearch } from "$lib/server/deepwell/search"
import { getRequestContext } from "$lib/server/load/request-ctx"
import type { PageServerLoad } from "./$types"

const PAGE_SIZE = 20
const MAX_OFFSET = 500
const MAX_QUERY_BYTES = 200

function searchOffset(value: string | null): number {
  if (value === null || !/^(0|[1-9]\d*)$/.test(value)) return 0
  const offset = Number(value)
  return Number.isSafeInteger(offset) && offset <= MAX_OFFSET ? offset : 0
}

function searchUrl(query: string, offset: number): string {
  const parameters = new URLSearchParams({ query, offset: String(offset) })
  return `/search:site?${parameters}`
}

export const load: PageServerLoad = async ({ url, locals, parent }) => {
  locals.documentLayout = (await parent()).site.layout
  const query = (url.searchParams.get("query") ?? "").trim()
  const offset = searchOffset(url.searchParams.get("offset"))
  const previousUrl =
    offset > 0 ? searchUrl(query, Math.max(0, offset - PAGE_SIZE)) : null

  if (!query) {
    return {
      query,
      hits: [],
      previousUrl,
      nextUrl: null,
      message: "Enter a search query."
    }
  }
  if (new TextEncoder().encode(query).length > MAX_QUERY_BYTES) {
    return {
      query,
      hits: [],
      previousUrl: null,
      nextUrl: null,
      message: "Search query must be 200 bytes or fewer."
    }
  }

  try {
    const results = await pageSearch(query, offset, PAGE_SIZE, getRequestContext(locals))
    const nextOffset = offset + PAGE_SIZE
    return {
      query,
      hits: results.hits,
      previousUrl,
      nextUrl:
        results.has_more && nextOffset <= MAX_OFFSET
          ? searchUrl(query, nextOffset)
          : null,
      message: results.hits.length ? null : "No results found."
    }
  } catch {
    return {
      query,
      hits: [],
      previousUrl: null,
      nextUrl: null,
      message: "Search is temporarily unavailable. Please try again later."
    }
  }
}
