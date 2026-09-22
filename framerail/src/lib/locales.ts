import { parse } from "accept-language-parser"

export function parseAcceptLangHeader(req: Request) {
  const language = req.headers.get("Accept-Language") ?? undefined
  const locales = parse(language)
    .sort((a, b) => b.quality - a.quality)
    .map((lang) => {
      const parts = [lang.code]
      if (lang.script) parts.push(lang.script)
      if (lang.region) parts.push(lang.region)
      return parts.join("-")
    })
  return locales
}
