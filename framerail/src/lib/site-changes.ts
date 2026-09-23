/**
 * Wikidot's SiteChanges form (`.site-changes-box`). Wikidot reloads the
 * list over AJAX; the replica encodes the choices in the page URL, which
 * Deepwell renders: `/<page>/p/1/perpage/N/category/C/types/NS`.
 */

export interface SiteChangesChoice {
  /** Flag letters of the checked revision types; empty means ALL. */
  types: string
  category: string
  perPage: number
}

export function siteChangesPath(page: string, choice: SiteChangesChoice): string {
  let path = `/${page}/p/1`
  if (choice.perPage !== 20) path += `/perpage/${choice.perPage}`
  if (choice.category) path += `/category/${encodeURIComponent(choice.category)}`
  if (choice.types) path += `/types/${choice.types}`
  return path
}

/**
 * The form's current choices; ALL wins over individual types, as on
 * Wikidot.
 */
export function readSiteChangesForm(box: Element): SiteChangesChoice {
  const all = box.querySelector<HTMLInputElement>("#rev-type-all")?.checked ?? true
  const types = all
    ? ""
    : [...box.querySelectorAll<HTMLInputElement>("input[data-flag]:checked")]
        .map((input) => input.dataset.flag)
        .join("")
  return {
    types,
    category: box.querySelector<HTMLSelectElement>("#rev-category")?.value ?? "",
    perPage: Number(box.querySelector<HTMLSelectElement>("#rev-perpage")?.value ?? 20)
  }
}

/** Document-level click listener for the form's "Update list" button. */
export function clickSiteChanges(event: MouseEvent, navigate: (path: string) => unknown) {
  const button = event.target
  if (!(button instanceof HTMLInputElement) || button.type !== "button") return
  const box = button.closest(".site-changes-box")
  if (!box) return
  event.preventDefault()
  const page = decodeURIComponent(window.location.pathname.split("/")[1] ?? "")
  navigate(siteChangesPath(page, readSiteChangesForm(box)))
}
