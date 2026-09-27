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

const MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec"
]
const enhancedDates = new WeakSet<Element>()

export function formatSiteChangeDate(timestamp: number, timeZone?: string): string {
  const parts = new Intl.DateTimeFormat("en-GB", {
    timeZone,
    day: "numeric",
    month: "numeric",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23"
  }).formatToParts(new Date(timestamp * 1000))
  const value = (part: string) => parts.find((item) => item.type === part)?.value ?? ""
  return `${Number(value("day"))} ${MONTHS[Number(value("month")) - 1]} ${value("year")} - ${value("hour")}:${value("minute")}:${value("second")}`
}

export function relativeSiteChangeDate(
  timestamp: number,
  now = Date.now() / 1000
): string {
  const elapsed = Math.max(1, Math.floor(now - timestamp))
  const unit =
    elapsed >= 86400
      ? "day"
      : elapsed >= 3600
        ? "hour"
        : elapsed >= 60
          ? "minute"
          : "second"
  const seconds = { day: 86400, hour: 3600, minute: 60, second: 1 }[unit]
  const amount = Math.floor(elapsed / seconds)
  return `${amount} ${unit}${amount === 1 ? "" : "s"} ago`
}

export function enhanceSiteChangeDates(
  root: ParentNode,
  now: () => number = () => Date.now() / 1000,
  timeZone?: string
): void {
  for (const time of root.querySelectorAll<HTMLElement>(
    ".site-changes-box time.site-change-date"
  )) {
    if (enhancedDates.has(time)) continue
    const raw = time.getAttribute("data-timestamp")
    if (!raw || !/^-?\d+$/.test(raw)) {
      throw new Error(`Invalid SiteChanges timestamp: ${raw}`)
    }
    const timestamp = Number(raw)
    if (
      !Number.isSafeInteger(timestamp) ||
      !Number.isFinite(timestamp * 1000) ||
      Number.isNaN(new Date(timestamp * 1000).getTime())
    ) {
      throw new Error(`Invalid SiteChanges timestamp: ${raw}`)
    }
    time.textContent = formatSiteChangeDate(timestamp, timeZone)
    const updateTitle = () =>
      time.setAttribute("title", relativeSiteChangeDate(timestamp, now()))
    time.addEventListener("mouseenter", updateTitle)
    time.addEventListener("focus", updateTitle)
    enhancedDates.add(time)
  }
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
