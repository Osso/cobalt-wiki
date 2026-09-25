export interface GallerySelection {
  images: { src: string; alt: string }[]
  initialIndex: number
  opener: HTMLAnchorElement
}

/** Read only ordinary clicks inside one compiled FTML gallery. */
export function readGallerySelection(event: MouseEvent): GallerySelection | null {
  if (
    event.defaultPrevented ||
    event.button !== 0 ||
    event.ctrlKey ||
    event.metaKey ||
    event.shiftKey ||
    event.altKey
  ) {
    return null
  }
  const opener =
    event.target instanceof Element
      ? event.target.closest<HTMLAnchorElement>(".gallery-item a[href]")
      : null
  const gallery = opener?.closest(".gallery-box")
  if (!opener || !gallery || !opener.querySelector("img")) return null

  const links = Array.from(
    gallery.querySelectorAll<HTMLAnchorElement>(".gallery-item a[href]")
  ).filter(
    (link) => link.closest(".gallery-box") === gallery && link.querySelector("img")
  )
  return {
    images: links.map((link) => ({
      src: link.href,
      alt: link.querySelector("img")?.alt || "Gallery image"
    })),
    initialIndex: links.indexOf(opener),
    opener
  }
}
