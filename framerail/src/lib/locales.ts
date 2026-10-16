function parseLanguagePreference(range: string) {
  const [tag, ...parameters] = range.trim().split(";")
  const weight = parameters
    .map((value) => value.trim())
    .find((value) => value.startsWith("q="))
  const quality = weight === undefined ? 1 : Number(weight.slice(2))
  if (!Number.isFinite(quality) || quality <= 0 || quality > 1 || tag === "*") {
    return []
  }

  try {
    // Deepwell accepts language identifiers, not Unicode preference extensions.
    const locale = new Intl.Locale(tag).baseName
    return [{ locale, quality }]
  } catch (error) {
    // An invalid client language range is not a usable translation preference.
    if (error instanceof RangeError) return []
    throw error
  }
}

export function parseAcceptLangHeader(req: Request) {
  const language = req.headers.get("Accept-Language") ?? ""
  return language
    .split(",")
    .flatMap(parseLanguagePreference)
    .sort((a, b) => b.quality - a.quality)
    .map(({ locale }) => locale)
}
