export type ImageCheckBounds = {
  width: number
  height: number
  left: number
  top: number
}

export function imageCheckBounds(
  imageWidth: number,
  imageHeight: number,
  availableWidth: number,
  availableHeight: number
): ImageCheckBounds {
  const width = Math.min(imageWidth + 200, availableWidth - 100)
  const height = Math.min(imageHeight + 200, availableHeight - 100)
  return {
    width,
    height,
    left: (availableWidth - width) / 2,
    top: (availableHeight - height) / 2
  }
}

function imageUrl(uri: string): URL {
  let url: URL
  try {
    url = new URL(uri)
  } catch {
    throw new Error("Enter a valid HTTP or HTTPS image URL.")
  }
  if (
    (url.protocol !== "http:" && url.protocol !== "https:") ||
    url.username ||
    url.password
  ) {
    throw new Error("Enter a valid HTTP or HTTPS image URL without credentials.")
  }
  return url
}

function bindImageCheckEvents(
  popup: Window,
  image: HTMLImageElement,
  status: HTMLParagraphElement
): void {
  image.addEventListener("load", () => {
    status.textContent = "Image loaded."
    const bounds = imageCheckBounds(
      image.naturalWidth,
      image.naturalHeight,
      screen.availWidth,
      screen.availHeight
    )
    popup.resizeTo(bounds.width, bounds.height)
    popup.moveTo(bounds.left, bounds.top)
  })
  image.addEventListener("error", () => {
    status.textContent = "Image unavailable."
  })
}

function addImageCheckContent(popup: Window, url: URL): void {
  const document = popup.document
  document.title = "Checking image..."
  const message = document.createElement("p")
  message.textContent =
    "If you see the image below, the image URL you entered is available."
  const image = document.createElement("img")
  image.id = "check-image"
  image.alt = "image not available!"
  const status = document.createElement("p")
  status.setAttribute("role", "status")
  status.textContent = "Checking image..."
  const close = document.createElement("a")
  close.href = "#"
  close.textContent = "close this window"
  close.addEventListener("click", (event) => {
    event.preventDefault()
    popup.close()
  })
  bindImageCheckEvents(popup, image, status)
  document.body.style.textAlign = "center"
  document.body.replaceChildren(message, image, status, close)
  image.src = url.href
}

export function openImageCheck(uri: string): void {
  const url = imageUrl(uri)
  const width = screen.width / 2
  const height = screen.height / 2
  const left = screen.width / 4
  const top = screen.height / 4
  const popup = window.open(
    "about:blank",
    "_blank",
    `location=no,menubar=no,titlebar=no,resizable=yes,scrollbars=yes,width=${width},height=${height},top=${top},left=${left}`
  )
  if (!popup) {
    throw new Error("Image check popup was blocked. Allow popups and try again.")
  }
  popup.opener = null
  addImageCheckContent(popup, url)
}
