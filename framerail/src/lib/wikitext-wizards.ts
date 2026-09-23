export type WizardKind = "table" | "code" | "uri" | "pageLink" | "image" | "eref"

export type WizardOptions =
  | { kind: "table"; rows: number; columns: number; headers: boolean }
  | { kind: "code"; type: string }
  | { kind: "uri"; uri: string; anchor: string; newWindow: boolean }
  | { kind: "pageLink"; page: string; anchor: string }
  | {
      kind: "image"
      source: "uri" | "file" | "flickr"
      value: string
      position: "" | "l" | "r" | "c" | "fl" | "fr"
      size?: string
    }
  | { kind: "eref"; label: string; withEq: boolean }

export type WizardResult = { value: string; start: number; end: number }
export type Equation = { label: string; source: string }

// Matches the source editor's line-start labelled [[math LABEL]] ... [[/math]] blocks.
export function extractEquations(value: string): Equation[] {
  const pattern = /^\[\[math\s([a-zA-Z0-9]+)\]\]\r?\n((?:.*\r?\n)*?)\[\[\/math\]\]/gm
  return Array.from(value.matchAll(pattern), ([, label, source]) => ({
    label,
    source: source.replace(/\r?\n$/, "")
  }))
}

export function normalizeFlickrSource(input: string): string {
  const page = /^https?:\/\/(?:www\.)?flickr\.com\/.*?\/([0-9]+)(?:\/.*)?$/.exec(input)
  const staticImage =
    /^https?:\/\/static\.flickr\.com\/[0-9]+\/([0-9]+)_([0-9a-z]+).*$/.exec(input)
  const id = staticImage?.[1] ?? page?.[1] ?? input
  if (!/^[0-9]+$/.test(id)) throw new Error("Invalid Flickr image ID or URL")
  return `flickr:${id}${staticImage ? `_${staticImage[2]}` : ""}`
}

function requireText(value: string, name: string, forbidden: RegExp): void {
  if (!value.trim() || forbidden.test(value)) throw new Error(`Invalid ${name}`)
}

function requireDimension(value: number, name: string): void {
  // Inference: the source dialog limits each numeric input to two characters, but does not validate it.
  const validInteger = Number.isInteger(value)
  const validRange = value >= 1 && value <= 99
  if (!validInteger || !validRange) {
    throw new RangeError(`${name} must be an integer in 1..99`)
  }
}

function tableText({
  rows,
  columns,
  headers
}: Extract<WizardOptions, { kind: "table" }>): string {
  requireDimension(rows, "rows")
  requireDimension(columns, "columns")
  return Array.from(
    { length: rows },
    (_, row) =>
      `\n||${Array.from({ length: columns }, () =>
        row === 0 && headers ? "~ header ||" : " cell-content ||"
      ).join("")}`
  ).join("")
}

function codeText(type: string): string {
  // Preserve the original dialog's JavaScipt value (sic), rather than silently rewriting it.
  const types = [
    "",
    "Cpp",
    "CSS",
    "PHP",
    "HTML",
    "diff",
    "Java",
    "JavaScipt",
    "Perl",
    "Python",
    "Ruby",
    "SQL",
    "XML"
  ]
  if (!types.includes(type)) throw new Error("Invalid code type")
  return `[[code${type ? ` type="${type}"` : ""}]]\n`
}

function imageText(options: Extract<WizardOptions, { kind: "image" }>): string {
  const source =
    options.source === "flickr" ? normalizeFlickrSource(options.value) : options.value
  requireText(source, "image source", /[\r\n[\]]/)
  if (options.source === "uri") requireText(source, "image URI", /\s/)
  const position = { "": "", l: "<", r: ">", c: "=", fl: "f<", fr: "f>" }[
    options.position
  ]
  if (position === undefined) throw new Error("Invalid image position")
  const size = options.size || ""
  if (size && /["\r\n[\]]/.test(size)) throw new Error("Invalid image size")
  return `[[${position}image ${source}${size ? ` size="${size}"` : ""}]]`
}

function insertAtStart(
  value: string,
  start: number,
  text: string,
  block: boolean
): WizardResult {
  let before = value.slice(0, start)
  let after = value.slice(start)
  if (block) {
    // insertText trims the caret range before applying these two source utilities.
    before = `${before.replace(/\r?\n$/, "")}\n`
    if (after) after = `\n\n${after.replace(/^\r?\n(\s*\r?\n)?/, "")}`
  }
  const prefix = `${before}${text}`
  return { value: `${prefix}${after}`, start: prefix.length, end: prefix.length }
}

function wrapCode(value: string, start: number, end: number, type: string): WizardResult {
  const selected = value.slice(start, end)
  const leading = /^\s*/.exec(selected)?.[0].length ?? 0
  const trailing = /\s*$/.exec(selected)?.[0].length ?? 0
  const from = start + leading
  const to = Math.max(from, end - trailing)
  const prefix = value.slice(0, from)
  const before = prefix ? `${prefix.replace(/(\r?\n\s*)?\r?\n$/, "")}\n\n` : ""
  const suffix = value.slice(to)
  const after = suffix ? `\n\n${suffix.replace(/^\r?\n(\s*\r?\n)?/, "")}` : ""
  const opening = codeText(type)
  const content = value.slice(from, to)
  const placeholder = "insert the code here"
  const body = content || placeholder
  const result = `${before}${opening}${body}\n[[/code]]`
  if (content) {
    return { value: result + after, start: result.length, end: result.length }
  }
  const selectionStart = before.length + opening.length
  return {
    value: result + after,
    start: selectionStart,
    end: selectionStart + placeholder.length
  }
}

function requireSelection(value: string, start: number, end: number): void {
  const validIntegers = Number.isInteger(start) && Number.isInteger(end)
  const validRange = start >= 0 && end >= start && end <= value.length
  if (!validIntegers || !validRange) throw new RangeError("Invalid wizard selection")
}

function uriText(options: Extract<WizardOptions, { kind: "uri" }>): string {
  requireText(options.uri, "URI", /\s|[[\]]/)
  if (/[\r\n[\]]/.test(options.anchor)) throw new Error("Invalid URI anchor")
  const marker = options.newWindow ? "*" : ""
  return options.anchor
    ? `[${marker}${options.uri} ${options.anchor}]`
    : `${marker}${options.uri}`
}

function pageLinkText(options: Extract<WizardOptions, { kind: "pageLink" }>): string {
  requireText(options.page, "page name", /[\r\n[\]]/)
  if (/[\r\n[\]]/.test(options.anchor)) throw new Error("Invalid page anchor")
  return `[[[${options.page}${options.anchor ? ` |${options.anchor}` : ""}]]]`
}

export function applyWizard(
  value: string,
  start: number,
  end: number,
  options: WizardOptions
): WizardResult {
  requireSelection(value, start, end)
  switch (options.kind) {
    case "table":
      return insertAtStart(value, start, tableText(options), true)
    case "code":
      return wrapCode(value, start, end, options.type)
    case "uri":
      return insertAtStart(value, start, uriText(options), false)
    case "pageLink":
      return insertAtStart(value, start, pageLinkText(options), false)
    case "image":
      return insertAtStart(value, start, imageText(options), false)
    case "eref": {
      if (!extractEquations(value).some(({ label }) => label === options.label)) {
        throw new Error(`Equation label not found: ${options.label}`)
      }
      const reference = `[[eref ${options.label}]]`
      return insertAtStart(
        value,
        start,
        options.withEq ? `Eq.(${reference})` : reference,
        false
      )
    }
  }
}
