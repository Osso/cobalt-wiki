import defaults from "$lib/defaults"
import { requireDeepwellError } from "$lib/deepwell-errors"
import {
  tagAddApply,
  tagAddPreview,
  tagApply,
  tagHistory,
  tagPreview,
  tagUndo
} from "$lib/server/deepwell/tag-admin"
import { translate } from "$lib/server/deepwell/translate"
import { loadSiteChrome } from "$lib/server/load/site-chrome"
import { loadSiteInfo } from "$lib/server/load/site-info"
import { fail } from "@sveltejs/kit"

import type {
  TagAddPreview,
  TagAddSelection,
  TagChange,
  TagOperation,
  TagPreview
} from "$lib/server/deepwell/tag-admin"
import type { PreloadDataAsync } from "$lib/server/deepwell/views"
import type { RequestContext } from "$lib/server/load/request-ctx"
import type { Cookies, RequestEvent } from "@sveltejs/kit"

const PERMISSION_DENIED = 3106
const ERRORS: Record<number, string> = {
  3106: "Only administrators of this site can manage tags.",
  4000: "Invalid tag operation. Check the tags or refresh operation history.",
  4304: "Pages changed since this preview. Preview again before confirming. Nothing was changed.",
  4305: "A page changed after this operation. Undo would overwrite a later edit, so nothing was changed.",
  4306: "An affected page is locked or cannot be edited. Nothing was changed."
}

export async function loadTagAdminPage(
  request: Request,
  cookies: Cookies,
  preloadData: PreloadDataAsync
) {
  const { siteId } = loadSiteInfo(request.headers)
  const sessionToken = cookies.get("wikijump_token")
  const parent = await preloadData()
  const chrome = await loadSiteChrome(siteId, sessionToken, parent)
  const internationalization = await translate(parent.locales, {
    ...defaults.translateKeys,
    ...chrome.footerKeys
  })
  let access: "admin" | "signed-out" | "denied" =
    parent.user_session?.user && sessionToken ? "admin" : "signed-out"
  let operations: TagOperation[] = []
  if (access === "admin") {
    try {
      operations = await tagHistory({ siteId, sessionToken })
    } catch (caught) {
      if (requireDeepwellError(caught).code !== PERMISSION_DENIED) throw caught
      access = "denied"
    }
  }
  return {
    access,
    operations,
    internationalization,
    compiled_top_bar_html: chrome.compiled_top_bar_html
  }
}

type ActionData = {
  saved: boolean
  message: string
  preview: TagPreview | TagAddPreview | null
}

async function runAction(
  event: RequestEvent,
  action: (form: FormData, context: RequestContext) => Promise<ActionData>
) {
  const sessionToken = event.cookies.get("wikijump_token")
  if (!sessionToken) {
    return fail(401, { saved: false, message: "You are not signed in.", preview: null })
  }
  const { siteId } = loadSiteInfo(event.request.headers)
  const form = await event.request.formData()
  try {
    return await action(form, { siteId, sessionToken })
  } catch (caught) {
    if (caught instanceof InvalidTagForm) {
      return fail(400, { saved: false, message: caught.message, preview: null })
    }
    const error = requireDeepwellError(caught)
    const status = errorStatus(error.code)
    return fail(status, {
      saved: false,
      message: ERRORS[error.code] ?? `Tag operation failed: ${error.message}`,
      preview: null
    })
  }
}

function errorStatus(code: number) {
  if (code === PERMISSION_DENIED) return 403
  return ERRORS[code] ? 400 : 500
}

class InvalidTagForm extends Error {}

function formText(form: FormData, name: string): string {
  const value = form.get(name)
  return typeof value === "string" ? value : ""
}

function readChange(form: FormData): TagChange {
  const source = formText(form, "sourceTag").trim()
  const operation = formText(form, "operation")
  const target = operation === "delete" ? null : formText(form, "targetTag").trim()
  const validOperation = operation === "rename" || operation === "delete"
  const validSource = isSingleTag(source)
  const validTarget = target === null || isSingleTag(target)
  if (!validOperation || !validSource || !validTarget) {
    throw new InvalidTagForm(
      "Enter one source tag and, for rename, one destination tag. Tags cannot contain spaces."
    )
  }
  if (source === target) {
    throw new InvalidTagForm("Choose a destination different from the source tag.")
  }
  return { source_tag: source, target_tag: target }
}

function readAddition(form: FormData): TagAddSelection {
  const tag = formText(form, "tag").trim()
  const existing_tag = formText(form, "existingTag").trim() || null
  const category = formText(form, "category").trim() || null
  const title_contains = formText(form, "titleContains").trim() || null
  const validTag = isSingleTag(tag)
  const validExistingTag = existing_tag === null || isSingleTag(existing_tag)
  if (!validTag || !validExistingTag) {
    throw new InvalidTagForm(
      "Enter one tag to add and, if filtering by tag, one existing tag. Tags cannot contain spaces."
    )
  }
  if (!existing_tag && !category && !title_contains) {
    throw new InvalidTagForm(
      "Choose at least one filter: existing tag, category, or title text."
    )
  }
  return { tag, existing_tag, category, title_contains }
}

function isSingleTag(tag: string) {
  return tag.length > 0 && !/\s/.test(tag)
}

function requireConfirmation(form: FormData) {
  if (formText(form, "confirmed") !== "yes") {
    throw new InvalidTagForm("Confirm the operation before changing tags.")
  }
}

export function tagPreviewAction(event: RequestEvent) {
  return runAction(event, async (form, context) => ({
    saved: false,
    message: "",
    preview:
      formText(form, "operation") === "add"
        ? await tagAddPreview(readAddition(form), context)
        : await tagPreview(readChange(form), context)
  }))
}

export function tagApplyAction(event: RequestEvent) {
  return runAction(event, async (form, context) => {
    requireConfirmation(form)
    const token = formText(form, "previewToken")
    if (!token) throw new InvalidTagForm("Preview the affected pages before confirming.")
    const ipAddress = event.getClientAddress()
    const result =
      formText(form, "operation") === "add"
        ? await tagAddApply(readAddition(form), token, ipAddress, context)
        : await tagApply(readChange(form), token, ipAddress, context)
    return {
      saved: true,
      message: `Tags changed on ${result.count} pages. Operation #${result.operation_id} recorded below.`,
      preview: null
    }
  })
}

export function tagUndoAction(event: RequestEvent) {
  return runAction(event, async (form, context) => {
    requireConfirmation(form)
    const id = Number(formText(form, "operationId"))
    if (!Number.isSafeInteger(id) || id <= 0) {
      throw new InvalidTagForm("Choose an operation to undo.")
    }
    const result = await tagUndo(id, event.getClientAddress(), context)
    return {
      saved: true,
      message: `Operation #${result.operation_id} undone on ${result.count} pages.`,
      preview: null
    }
  })
}
