// The backend accepts avatars up to 250 KiB; a phone photo is several MB.
// Crop to a centered square and scale down in the browser before upload.
export const AVATAR_SIZE = 256

export async function squareAvatar(file: File, size = AVATAR_SIZE): Promise<File> {
  const bitmap = await createImageBitmap(file)
  const side = Math.min(bitmap.width, bitmap.height)
  const canvas = new OffscreenCanvas(size, size)
  const context = canvas.getContext("2d")
  if (!context) throw new Error("Canvas 2D is unavailable")
  context.drawImage(
    bitmap,
    (bitmap.width - side) / 2,
    (bitmap.height - side) / 2,
    side,
    side,
    0,
    0,
    size,
    size
  )
  bitmap.close()
  const blob = await canvas.convertToBlob({ type: "image/webp", quality: 0.9 })
  return new File([blob], "avatar.webp", { type: "image/webp" })
}
