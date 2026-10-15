/**
 * Character and background-character pages, except each category's
 * `_template`.
 */
export function isCharacterPage(slug: string): boolean {
  return /^(?:character|bgc):(?!_template$)./.test(slug)
}
