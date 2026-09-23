export type ToolbarResult = { value: string; start: number; end: number }

type Formatting = {
  before: string
  after?: string
  placeholder?: string
  block?: "paragraph" | "heading" | "line"
  multiline?: string
  insertOnly?: boolean
}

const formatting: Record<string, Formatting> = {
  bold: { before: "**", after: "**", placeholder: "bold text" },
  italic: { before: "//", after: "//", placeholder: "italic text" },
  underline: { before: "__", after: "__", placeholder: "underline text" },
  strikethrough: { before: "--", after: "--", placeholder: "strikethrough text" },
  teletype: { before: "{{", after: "}}", placeholder: "teletype text" },
  superscript: { before: "^^", after: "^^", placeholder: "superscript" },
  subscript: { before: ",,", after: ",,", placeholder: "subscript" },
  raw: { before: "@@", after: "@@", placeholder: "raw text" },
  quote: { before: "> ", placeholder: "quoted text", block: "line", multiline: "> " },
  numberedList: {
    before: "# ",
    placeholder: "list item",
    block: "line",
    multiline: "# "
  },
  bulletedList: {
    before: "* ",
    placeholder: "list item",
    block: "line",
    multiline: "* "
  },
  definitionList: {
    before: ": ",
    after: " : definition",
    placeholder: "item",
    block: "line",
    multiline: "* "
  },
  hr: { before: "------", block: "paragraph", insertOnly: true },
  clearFloat: { before: "~~~~", block: "paragraph", insertOnly: true },
  clearFloatLeft: { before: "~~~~<", block: "paragraph", insertOnly: true },
  clearFloatRight: { before: "~~~~>", block: "paragraph", insertOnly: true },
  toc: { before: "[[toc]]", block: "line", insertOnly: true },
  uri: { before: "[https://www.example.com ", after: "]", placeholder: "describe link" },
  pageLink: { before: "[[[", after: "]]]", placeholder: "page name" },
  image: { before: "[[image ", after: "]]", placeholder: "source" },
  footnote: {
    before: "[[footnote]] ",
    after: " [[/footnote]]",
    placeholder: "footnote text"
  },
  inlineMath: {
    before: "[[$ ",
    after: " $]]",
    placeholder: "insert LaTeX equation here"
  },
  bibliographycitation: { before: "[((bibcite ", after: "))]", placeholder: "label" },
  div: {
    before: "[[div]]\n",
    after: "\n[[/div]]",
    placeholder: "block contents",
    block: "paragraph"
  },
  code: {
    before: "[[code]]\n",
    after: "\n[[/code]]",
    placeholder: "insert the code here",
    block: "paragraph"
  },
  html: {
    before: "[[html]]\n",
    after: "\n[[/html]]",
    placeholder: "Insert any HTML code, including widgets and video or audio players",
    block: "paragraph"
  },
  math: {
    before: "[[math]]\n",
    after: "\n[[/math]]",
    placeholder: "insert LaTeX equation here",
    block: "paragraph"
  },
  bibliography: {
    before: "[[bibliography]]\n: ",
    after: " : full source reference\n[[/bibliography]]",
    placeholder: "label",
    block: "paragraph"
  }
}

function separateBefore(text: string, block: Formatting["block"]): string {
  if (!block) return text
  if (block === "heading") return text ? `${text.trimEnd()}\n\n` : ""
  if (block === "paragraph")
    return text ? `${text.replace(/(\r?\n\s*)?\r?\n$/, "")}\n\n` : ""
  return `${text.replace(/\r?\n$/, "")}\n`
}

function separateAfter(text: string, block: Formatting["block"]): string {
  if (block === "heading") return `\n\n${text.trimStart()}`
  if (!block || !text) return text
  if (block === "paragraph") return `\n\n${text.replace(/^\r?\n(\s*\r?\n)?/, "")}`
  return `\n${text.replace(/^\r?\n/, "")}`
}

function changeListIndent(text: string, increase: boolean): string {
  if (!increase) return text.replace(/(\r?\n\s*) ([*#].*)$/, "$1$2")
  if (/\r?\n(\s*)[*#].*\r?\n\1\s+[*#].*$/.test(text)) return text
  return text.replace(/(\r?\n\s*[*#].*)(\r?\n\s*)([*#].*)$/, "$1$2 $3")
}

export function applyWikitextToolbar(
  value: string,
  start: number,
  end: number,
  control: string
): ToolbarResult {
  if (start < 0 || end < start || end > value.length) {
    throw new RangeError("Invalid toolbar selection")
  }
  if (control === "increaseListIndent" || control === "decreaseListIndent") {
    const prefix = changeListIndent(
      value.slice(0, start),
      control === "increaseListIndent"
    )
    const text = prefix + value.slice(start)
    return { value: text, start: prefix.length, end: prefix.length }
  }
  const spec = /^heading([1-6])$/.exec(control)
  const format = spec
    ? {
        before: `${"+".repeat(Number(spec[1]))} `,
        placeholder: `heading level ${spec[1]}`,
        block: "heading" as const
      }
    : formatting[control]
  if (!format) throw new Error(`Unknown wikitext toolbar control: ${control}`)

  const selected = value.slice(start, end)
  const leading = selected.match(/^\s*/)?.[0].length ?? 0
  const trailing = selected.match(/\s*$/)?.[0].length ?? 0
  const from = start + leading
  const to = Math.max(from, end - trailing)
  const before = separateBefore(value.slice(0, from), format.block)
  const after = separateAfter(value.slice(to), format.block)
  const original = value.slice(from, to)
  const content = format.multiline
    ? original.replace(/\r?\n/g, `\n${format.multiline}`)
    : original
  const inserted = content || (format.insertOnly ? "" : format.placeholder || "")
  const wrapped = format.before + inserted + (format.after || "")
  const selectionStart =
    before.length +
    format.before.length +
    (content ? wrapped.length - format.before.length : 0)
  const selectionEnd =
    content || format.insertOnly ? selectionStart : selectionStart + inserted.length
  const cursor = format.insertOnly ? before.length + wrapped.length : selectionStart
  return {
    value: before + wrapped + after,
    start: cursor,
    end: format.insertOnly ? cursor : selectionEnd
  }
}
