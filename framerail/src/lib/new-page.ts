/**
 * Wikidot's NewPage module form (`.new-page-box`), as handled by its
 * NewPageHelperModule script and NewPageHelperAction: validate the name,
 * build the page name, then open the new page's editor with the typed
 * title.
 */

/**
 * WDStringUtils::toUnixName; diacritics are stripped like its
 * transliteration table.
 */
export function toUnixName(text: string): string {
  return text
    .trim()
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .replace(/[^a-z0-9\-:_]/g, "-")
    .replace(/^_/, ":_")
    .replace(/(?<!:)_/g, "-")
    .replace(/^-*/, "")
    .replace(/-*$/, "")
    .replace(/-{2,}/g, "-")
    .replace(/:{2,}/g, ":")
    .replaceAll(":-", ":")
    .replaceAll("-:", ":")
    .replaceAll("_-", "_")
    .replaceAll("-_", "_")
    .replace(/^:/, "")
    .replace(/:$/, "")
}

/** A PHP `/pattern/flags` regular expression as a JavaScript one. */
function parseFormat(format: string): RegExp {
  const end = format.lastIndexOf(format[0])
  const flags = format.slice(end + 1).replace(/[^imsu]/g, "")
  return new RegExp(format.slice(1, end), flags)
}

export type NewPageTarget = { path: string; unixName: string } | { error: string }

export function newPageTarget(
  pageName: string,
  categoryName: string,
  format: string
): NewPageTarget {
  const name = pageName.trim()
  if (!name) return { error: "You should provide a page name." }
  if (format.trim() && !parseFormat(format.trim()).test(name)) {
    return { error: "The page name is not in the required format." }
  }
  const unixName = toUnixName(`${categoryName.trim()}:${name}`)
  return { unixName, path: `/${unixName}/edit/true/title/${encodeURIComponent(name)}` }
}

/** Document-level submit listener for NewPage forms in rendered page HTML. */
export async function submitNewPage(
  event: SubmitEvent,
  navigate: (path: string) => unknown,
  showError: (message: string) => void
): Promise<void> {
  const form = event.target
  if (!(form instanceof HTMLFormElement) || !form.closest(".new-page-box")) return
  event.preventDefault()
  const field = (name: string) =>
    (form.elements.namedItem(name) as HTMLInputElement | null)?.value ?? ""
  const target = newPageTarget(field("pageName"), field("categoryName"), field("format"))
  if ("error" in target) return showError(target.error)
  const existing = await fetch(`/${target.unixName}`, { method: "HEAD" })
  if (existing.status !== 404) {
    return showError(`The page ${target.unixName} already exists.`)
  }
  navigate(target.path)
}
