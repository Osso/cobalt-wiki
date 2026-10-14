/** Accept an explicit, root-relative return path on this origin only. */
export function parseLoginReturnPath(url: URL): string | undefined {
  const original = url.searchParams.get("origUrl")
  if (!original?.startsWith("/") || original.startsWith("//")) return undefined

  let target: URL
  try {
    target = new URL(original, url)
  } catch {
    return undefined
  }
  if (target.origin !== url.origin) return undefined
  return `${target.pathname}${target.search}${target.hash}`
}
