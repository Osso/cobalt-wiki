import { deserialize } from "$app/forms"

export type UploadRow = {
  file: File
  state: "queued" | "uploading" | "uploaded" | "failed"
  error?: string
}

type Identity = { siteId: number; pageId: number; lastRevisionId: number }

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

async function sendUpload(
  url: string,
  form: FormData,
  fetchUpload: UploadOptions["fetchUpload"]
) {
  try {
    const response = await fetchUpload(url, { method: "POST", body: form })
    const result = deserialize<
      { res: unknown },
      { message?: string; form?: { errors?: Record<string, unknown> } }
    >(await response.text())
    if (result.type === "success") {
      return hasFileReceipt(result.data?.res)
        ? undefined
        : "Upload response did not identify a stored file."
    }
    if (result.type === "failure") return uploadError(result.data, result.status)
    return uploadError(result.type === "error" ? result.error : null, result.status)
  } catch (error) {
    return error instanceof Error ? error.message : String(error)
  }
}

export async function uploadFiles(options: UploadOptions): Promise<UploadRow[]> {
  let rows: UploadRow[] = options.files.map((file) => ({ file, state: "queued" }))
  options.onStatus(rows)
  for (const [index, file] of options.files.entries()) {
    if (!options.isActive()) break
    rows = rows.map((row, current) =>
      current === index ? { ...row, state: "uploading" } : row
    )
    options.onStatus(rows)
    const name = options.files.length === 1 ? options.name : file.name
    const form = buildUploadFormData(file, options.identity, name, options.comments)
    const error = await sendUpload(options.url, form, options.fetchUpload)
    rows = rows.map((row, current) =>
      current === index
        ? error !== undefined
          ? { ...row, state: "failed", error }
          : { ...row, state: "uploaded" }
        : row
    )
    options.onStatus(rows)
  }
  if (rows.some((row) => row.state === "uploaded")) await options.onUploaded()
  return rows
}
