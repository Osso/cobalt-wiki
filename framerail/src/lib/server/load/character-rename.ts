import { deepwellErrorStatus, requireDeepwellError } from "$lib/deepwell-errors"
import {
  characterRenamePreview,
  characterRenameStart,
  characterRenameStatus
} from "$lib/server/deepwell/character-rename"
import { getRequestContext } from "$lib/server/load/request-ctx"
import { fail } from "@sveltejs/kit"

import type { RequestContext } from "$lib/server/load/request-ctx"
import type { RequestEvent } from "@sveltejs/kit"

const PERMISSION_DENIED = 3106
/** Invalid name or not a character page; Deepwell's message says which. */
const BAD_REQUEST = 4000
const ERRORS: Record<number, string> = {
  3106: "You don't have permission to rename this character or one of the pages that mention it",
  2104: "A character page with that name already exists",
  2105: "An icon with that name already exists",
  4304: "Affected pages changed; preview again",
  4306: "One of the affected pages is locked or not editable",
  4307: "Reload this page, then preview and confirm keeping existing destination tags"
}

class InvalidRenameForm extends Error {}

async function runAction<T>(
  event: RequestEvent,
  action: (form: FormData, context: RequestContext) => Promise<T>
) {
  const form = await event.request.formData()
  try {
    return await action(form, getRequestContext(event.locals))
  } catch (caught) {
    if (caught instanceof InvalidRenameForm) {
      return fail(400, { message: caught.message })
    }
    const error = requireDeepwellError(caught)
    return fail(errorStatus(error.code), {
      message: ERRORS[error.code] ?? error.message
    })
  }
}

function errorStatus(code: number) {
  if (code === PERMISSION_DENIED) return 403
  if (code === BAD_REQUEST || ERRORS[code]) return 400
  return deepwellErrorStatus(code)
}

function formText(form: FormData, name: string): string {
  const value = form.get(name)
  return typeof value === "string" ? value.trim() : ""
}

function readId(form: FormData, name: string): number {
  const id = Number(formText(form, name))
  if (!Number.isSafeInteger(id) || id <= 0) {
    throw new InvalidRenameForm(`Invalid ${name}.`)
  }
  return id
}

function readName(form: FormData): string {
  const name = formText(form, "newName")
  if (!name) throw new InvalidRenameForm("Enter the new character name.")
  return name
}

export function characterRenamePreviewAction(event: RequestEvent) {
  return runAction(event, async (form, context) => ({
    preview: await characterRenamePreview(readId(form, "pageId"), readName(form), context)
  }))
}

export function characterRenameStartAction(event: RequestEvent) {
  return runAction(event, async (form, context) => {
    const previewToken = formText(form, "previewToken")
    if (formText(form, "confirmed") !== "yes" || !previewToken) {
      throw new InvalidRenameForm("Preview the rename and confirm it before starting.")
    }
    const { operation_id } = await characterRenameStart(
      readId(form, "pageId"),
      readName(form),
      previewToken,
      formText(form, "allowExistingTags") === "yes",
      event.getClientAddress(),
      context
    )
    return { operationId: operation_id }
  })
}

export function characterRenameStatusAction(event: RequestEvent) {
  return runAction(event, async (form, context) => ({
    progress: await characterRenameStatus(readId(form, "operationId"), context)
  }))
}
