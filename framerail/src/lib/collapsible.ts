/**
 * Wikidot's collapsible block: clicking either link swaps the folded and
 * unfolded parts, as Wikidot's collapsible JavaScript does.
 */
export function clickCollapsible(event: MouseEvent) {
  const link = event.target instanceof Element ? event.target.closest("a") : null
  if (!link?.classList.contains("collapsible-block-link")) return
  const block = link.closest(".collapsible-block")
  const folded = block?.querySelector<HTMLElement>(":scope > .collapsible-block-folded")
  const unfolded = block?.querySelector<HTMLElement>(
    ":scope > .collapsible-block-unfolded"
  )
  if (!folded || !unfolded) return
  event.preventDefault()
  const open = unfolded.style.display === "none"
  unfolded.style.display = open ? "block" : "none"
  folded.style.display = open ? "none" : "block"
}
