import { fail } from "@sveltejs/kit"
import {
  array,
  boolean,
  check,
  integer,
  minValue,
  null_,
  number,
  optional,
  pipe,
  record,
  safeParse,
  strictObject,
  string,
  union
} from "valibot"

import { pagePreview } from "$lib/server/deepwell/page-preview"
import type { PagePreviewRequest } from "$lib/server/deepwell/page-preview"
import type { RequestEvent } from "@sveltejs/kit"
import { getRequestContext } from "./request-ctx"

const previewSchema = pipe(
  strictObject({
    title: optional(string()),
    alt_title: optional(union([string(), null_()])),
    tags: optional(array(string())),
    wikitext: optional(string()),
    form_updates: optional(
      record(string(), union([string(), number(), boolean(), null_()]))
    ),
    last_revision_id: optional(pipe(number(), integer(), minValue(0)))
  }),
  check(
    (value) => (value.wikitext === undefined) !== (value.form_updates === undefined),
    "Submit either wikitext or form updates, not both"
  )
)

export async function pagePreviewAction({ request, locals }: RequestEvent) {
  let body: unknown
  try {
    body = await request.json()
  } catch {
    return fail(400, { message: "Invalid preview request" })
  }
  const parsed = safeParse(previewSchema, body)
  if (!parsed.success) {
    return fail(400, { message: "Invalid preview request" })
  }

  try {
    const result = await pagePreview(
      parsed.output as PagePreviewRequest,
      getRequestContext(locals)
    )
    if (typeof result?.html !== "string") {
      return fail(500, { message: "Unable to preview page" })
    }
    return result
  } catch {
    return fail(500, { message: "Unable to preview page" })
  }
}
