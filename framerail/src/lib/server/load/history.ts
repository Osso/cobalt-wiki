import { fail } from "@sveltejs/kit"
import { client } from "$lib/server/deepwell"
import { pageView } from "$lib/server/deepwell/views"
import { requireDeepwellError } from "$lib/deepwell-errors"
import { loadSiteInfo } from "./site-info"
import { getRequestContext } from "./request-ctx"

import type { RequestEvent } from "@sveltejs/kit"

export type HistoryOrigin = "wikidot" | "local"
export type HistoryFilters = Record<
  "all" | "source" | "title" | "move" | "tags" | "meta" | "files",
  boolean
>
export interface HistoryRow {
  id: number
  number: number
  flags: string[]
  author_id: number | null
  author_name: string | null
  author_slug: string | null
  created_at: string
  comments: string
  is_current: boolean
  representation: string | null
}
export interface HistoryList {
  origin: HistoryOrigin
  page: number
  per_page: number
  total: number
  total_pages: number
  available: { wikidot: boolean; local: boolean }
  rows: HistoryRow[]
}
export interface HistoryRevision {
  id: number
  number: number
  source: string
  rendered_html: string | null
  representation: string | null
}
export interface HistoryComparison {
  from: number
  to: number
  lines: { kind: "same" | "delete" | "insert"; text: string }[]
  representation: string | null
}

const filterNames = ["all", "source", "title", "move", "tags", "meta", "files"] as const
const pageSizes = [10, 20, 50, 100, 200]
const defaultFilters: HistoryFilters = {
  all: true,
  source: false,
  title: false,
  move: false,
  tags: false,
  meta: false,
  files: false
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
}

function origin(value: unknown): value is HistoryOrigin {
  return value === "wikidot" || value === "local"
}

function integer(value: unknown, minimum: number): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= minimum
}

function parseFilters(value: unknown): HistoryFilters | null {
  if (value === undefined) return defaultFilters
  if (
    !record(value) ||
    Object.keys(value).some(
      (key) => !filterNames.includes(key as (typeof filterNames)[number])
    )
  ) {
    return null
  }
  if (filterNames.some((key) => typeof value[key] !== "boolean")) return null
  return value as unknown as HistoryFilters
}

function parseList(value: unknown) {
  if (!record(value) || !origin(value.origin)) return null
  const page = value.page ?? 1
  const perPage = value.per_page ?? 20
  const filters = parseFilters(value.filters)
  if (!integer(page, 1) || !pageSizes.includes(perPage as number) || !filters) return null
  return { origin: value.origin, page, per_page: perPage as number, filters }
}

function parseRevision(value: unknown) {
  if (!record(value) || !origin(value.origin) || !integer(value.number, 0)) return null
  if (value.rendered !== undefined && typeof value.rendered !== "boolean") return null
  return { origin: value.origin, number: value.number, rendered: value.rendered ?? false }
}

function parseCompare(value: unknown) {
  if (
    !record(value) ||
    !origin(value.origin) ||
    !integer(value.from, 0) ||
    !integer(value.to, 0) ||
    value.from >= value.to
  ) {
    return null
  }
  return { origin: value.origin, from: value.from, to: value.to }
}

async function resolvePage(
  event: RequestEvent
): Promise<{ status: 403 | 404 } | { site_id: number; page_id: number }> {
  const { siteId } = loadSiteInfo(event.request.headers)
  const view = await pageView(
    siteId,
    [],
    event.params.slug ? { slug: event.params.slug, extra: event.params.extra } : null,
    event.cookies.get("wikijump_token")
  )
  if (view.type === "permissions") return { status: 403 as const }
  if (view.type !== "found") return { status: 404 as const }
  return { site_id: siteId, page_id: view.data.page.page_id }
}

async function readHistory<T>(
  event: RequestEvent,
  method: string,
  input: Record<string, unknown>
) {
  try {
    const page = await resolvePage(event)
    if ("status" in page) {
      return fail(page.status, { message: "Page history unavailable" })
    }
    const res: T = await client.request(
      method,
      { ...page, ...input },
      getRequestContext(event.locals)
    )
    return { res }
  } catch (cause) {
    return fail(500, requireDeepwellError(cause))
  }
}

async function parseRequest(event: RequestEvent): Promise<unknown> {
  try {
    return await event.request.json()
  } catch {
    return null
  }
}

export async function historyListAction(event: RequestEvent) {
  const input = parseList(await parseRequest(event))
  if (!input) return fail(400, { message: "Invalid history list request" })
  return readHistory<HistoryList>(event, "page_history_list", input)
}

export async function historyRevisionAction(event: RequestEvent) {
  const input = parseRevision(await parseRequest(event))
  if (!input) return fail(400, { message: "Invalid history revision request" })
  return readHistory<HistoryRevision | null>(event, "page_history_revision", input)
}

export async function historyCompareAction(event: RequestEvent) {
  const input = parseCompare(await parseRequest(event))
  if (!input) return fail(400, { message: "Invalid history comparison request" })
  return readHistory<HistoryComparison>(event, "page_history_compare", input)
}
