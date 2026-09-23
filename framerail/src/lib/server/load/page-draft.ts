import { fail } from "@sveltejs/kit"
import {
  boolean,
  check,
  integer,
  minValue,
  null_,
  number,
  object,
  optional,
  pipe,
  record,
  safeParse,
  strictObject,
  string,
  union
} from "valibot"

import {
  pageDraftDelete,
  pageDraftGet,
  pageDraftSave
} from "$lib/server/deepwell/page-draft"
import type { PageDraftRequest } from "$lib/server/deepwell/page-draft"
import type { RequestEvent } from "@sveltejs/kit"
import { getRequestContext } from "./request-ctx"

const scalar = union([string(), number(), boolean(), null_()])
const draftSchema = object({
  title: string(),
  wikitext: string(),
  updated_at: string(),
  form_values: optional(record(string(), scalar))
})
const responseSchema = object({ draft: union([draftSchema, null_()]) })
const deleteSchema = object({ deleted: boolean() })
const saveSchema = pipe(
  strictObject({
    title: string(),
    wikitext: optional(string()),
    form_updates: optional(record(string(), scalar)),
    last_revision_id: optional(pipe(number(), integer(), minValue(0)))
  }),
  check(
    (value) => (value.wikitext === undefined) !== (value.form_updates === undefined),
    "Submit either wikitext or form updates, not both"
  )
)

async function readDraftPayload(request: Request): Promise<unknown> {
  const form = await request.formData()
  if (form.has("payload") && Array.from(form.keys()).length === 1) {
    const payload = form.get("payload")
    if (typeof payload === "string") return JSON.parse(payload)
  }
  throw new Error("Invalid draft request")
}

async function hasEmptyPayload(request: Request): Promise<boolean> {
  const form = await request.formData()
  return Array.from(form.keys()).length === 0
}

export async function pageDraftGetAction({ request, locals }: RequestEvent) {
  try {
    if (!(await hasEmptyPayload(request))) {
      return fail(400, { message: "Invalid draft request" })
    }
  } catch {
    return fail(400, { message: "Invalid draft request" })
  }

  try {
    const result = await pageDraftGet(getRequestContext(locals))
    const parsed = safeParse(responseSchema, result)
    if (parsed.success) return parsed.output
  } catch {
    // Backend errors must not expose private draft contents.
  }
  return fail(500, { message: "Unable to load draft" })
}

export async function pageDraftSaveAction({ request, locals }: RequestEvent) {
  let payload: unknown
  try {
    payload = await readDraftPayload(request)
  } catch {
    return fail(400, { message: "Invalid draft request" })
  }
  const parsedRequest = safeParse(saveSchema, payload)
  if (!parsedRequest.success) {
    return fail(400, { message: "Invalid draft request" })
  }

  try {
    const result = await pageDraftSave(
      parsedRequest.output as PageDraftRequest,
      getRequestContext(locals)
    )
    const parsedResponse = safeParse(responseSchema, result)
    if (parsedResponse.success && parsedResponse.output.draft !== null) {
      return parsedResponse.output
    }
  } catch {
    // Backend errors must not expose private draft contents.
  }
  return fail(500, { message: "Unable to save draft" })
}

export async function pageDraftDeleteAction({ request, locals }: RequestEvent) {
  try {
    if (!(await hasEmptyPayload(request))) {
      return fail(400, { message: "Invalid draft request" })
    }
  } catch {
    return fail(400, { message: "Invalid draft request" })
  }

  try {
    const result = await pageDraftDelete(getRequestContext(locals))
    const parsed = safeParse(deleteSchema, result)
    if (parsed.success) return parsed.output
  } catch {
    // Backend errors must not expose private draft contents.
  }
  return fail(500, { message: "Unable to delete draft" })
}
