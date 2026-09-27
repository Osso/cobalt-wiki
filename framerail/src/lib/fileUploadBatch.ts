import { deserialize } from "$app/forms"

export type UploadRow = {
  file: File
  /** The name the file is stored under on the page. */
  name: string
  state: "queued" | "uploading" | "uploaded" | "failed" | "replacing" | "replaced"
  error?: string
}

type Identity = { siteId: number; pageId: number; lastRevisionId: number }

/** A file already on the page, as listed in the Files pane. */
export type ExistingFile = { file_id: number; revision_id: number; name: string }

type UploadOptions = {
  files: File[]
  identity: Identity
  name: string
  comments: string
  url: string
  fetchUpload: (url: string, init: RequestInit) => Promise<Response>
  isActive: () => boolean
  onStatus: (rows: UploadRow[]) => void
  onUploaded: () => Promise<void>
}

export function buildUploadFormData(
  file: File,
  identity: Identity,
  name: string,
  comments: string
): FormData {
  const form = new FormData()
  form.set("siteId", String(identity.siteId))
  form.set("pageId", String(identity.pageId))
  form.set("lastRevisionId", String(identity.lastRevisionId))
  form.set("file", file)
  form.set("name", name)
  form.set("comments", comments)
  return form
}

/**
 * Replaces an existing file's bytes under its current name.
 * `lastRevisionId` is the file's revision, which the edit action checks
 * for conflicts.
 */
export function buildReplaceFormData(
  file: File,
  identity: Identity,
  existing: ExistingFile,
  comments: string
): FormData {
  const form = new FormData()
  form.set("siteId", String(identity.siteId))
  form.set("pageId", String(identity.pageId))
  form.set("lastRevisionId", String(existing.revision_id))
  form.set("fileId", String(existing.file_id))
  form.set("file", file)
  form.set("name", existing.name)
  form.set("comments", comments)
  return form
}

function uploadError(data: unknown, status: number): string {
  if (data && typeof data === "object") {
    const payload = data as {
      message?: unknown
      form?: { errors?: Record<string, unknown> }
    }
    if (typeof payload.message === "string" && payload.message) return payload.message
    const errors = payload.form?.errors
    if (errors) {
      const messages = Object.values(errors).flatMap((value) =>
        Array.isArray(value)
          ? value.filter((item): item is string => typeof item === "string")
          : []
      )
      if (messages.length) return messages.join("; ")
    }
  }
  return `Upload failed (status ${status})`
}

function hasFileReceipt(value: unknown): boolean {
  if (!value || typeof value !== "object" || !("file_id" in value)) return false
  return (
    typeof value.file_id === "number" &&
    Number.isSafeInteger(value.file_id) &&
    value.file_id > 0
  )
}

// An edit returns the new revision, or null when the bytes were already identical.
function hasRevisionReceipt(value: unknown): boolean {
  if (value === null) return true
  if (!value || typeof value !== "object" || !("file_revision_id" in value)) return false
  return (
    typeof value.file_revision_id === "number" &&
    Number.isSafeInteger(value.file_revision_id) &&
    value.file_revision_id > 0
  )
}

async function sendUpload(
  url: string,
  form: FormData,
  fetchUpload: UploadOptions["fetchUpload"],
  hasReceipt: (value: unknown) => boolean = hasFileReceipt
) {
  try {
    const response = await fetchUpload(url, { method: "POST", body: form })
    const result = deserialize<
      { res: unknown },
      { message?: string; form?: { errors?: Record<string, unknown> } }
    >(await response.text())
    if (result.type === "success") {
      return hasReceipt(result.data?.res)
        ? undefined
        : "Upload response did not identify a stored file."
    }
    if (result.type === "failure") return uploadError(result.data, result.status)
    return uploadError(result.type === "error" ? result.error : null, response.status)
  } catch (error) {
    return error instanceof Error ? error.message : String(error)
  }
}

function updateUploadRow(
  rows: UploadRow[],
  index: number,
  update: Pick<UploadRow, "state" | "error">
): UploadRow[] {
  return rows.map((row, current) => (current === index ? { ...row, ...update } : row))
}

export async function uploadFiles(options: UploadOptions): Promise<UploadRow[]> {
  const single = options.files.length === 1
  let rows: UploadRow[] = options.files.map((file) => ({
    file,
    name: (single && options.name) || file.name,
    state: "queued"
  }))
  options.onStatus(rows)
  for (const [index, file] of options.files.entries()) {
    if (!options.isActive()) break
    rows = updateUploadRow(rows, index, { state: "uploading" })
    options.onStatus(rows)
    const name = single ? options.name : file.name
    const form = buildUploadFormData(file, options.identity, name, options.comments)
    const error = await sendUpload(options.url, form, options.fetchUpload)
    const outcome: Pick<UploadRow, "state" | "error"> =
      error === undefined ? { state: "uploaded" } : { state: "failed", error }
    rows = updateUploadRow(rows, index, outcome)
    options.onStatus(rows)
  }
  if (rows.some((row) => row.state === "uploaded")) await options.onUploaded()
  return rows
}

/**
 * Replaces an existing page file with an upload that collided with its
 * name.
 */
export async function replaceFile(options: {
  file: File
  existing: ExistingFile
  identity: Identity
  comments: string
  url: string
  fetchUpload: UploadOptions["fetchUpload"]
}): Promise<Pick<UploadRow, "state" | "error">> {
  const form = buildReplaceFormData(
    options.file,
    options.identity,
    options.existing,
    options.comments
  )
  const error = await sendUpload(
    options.url,
    form,
    options.fetchUpload,
    hasRevisionReceipt
  )
  return error === undefined ? { state: "replaced" } : { state: "failed", error }
}
