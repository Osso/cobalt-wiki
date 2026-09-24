/**
 * Wikidot's tag buttons (`<a class="wiki-standalone-button">`): `set-tags`
 * applies its `data-tags` changes to the current page, `tags` opens the
 * tag editor.
 */

/**
 * Applies Wikidot's space-separated tag changes in order: `+x` adds `x`,
 * `-x` removes it, other words are ignored. `-@@` removes the literal tag
 * `@@`, which an empty `%%form_raw{...}%%` field leaves behind.
 */
export function applyTagChanges(tags: readonly string[], changes: string): string[] {
  const result = [...tags]
  for (const word of changes.split(/\s+/)) {
    const tag = word.slice(1)
    if (!tag) continue
    const index = result.indexOf(tag)
    if (word.startsWith("+") && index === -1) result.push(tag)
    if (word.startsWith("-") && index !== -1) result.splice(index, 1)
  }
  return result
}

/** The changes that turn `before` into `after`, e.g. `-old +new`. */
export function tagChangesBetween(
  before: readonly string[],
  after: readonly string[]
): string {
  const removed = before.filter((tag) => !after.includes(tag)).map((tag) => `-${tag}`)
  const added = after.filter((tag) => !before.includes(tag)).map((tag) => `+${tag}`)
  return [...removed, ...added].join(" ")
}

export interface TagButtonActions {
  setTags(changes: string): void
  openTags(): void
}

/** Window-level click listener for `set-tags` and `tags` buttons. */
export function clickTagButton(event: MouseEvent, actions: TagButtonActions) {
  const link = event.target instanceof Element ? event.target.closest("a") : null
  if (!link?.classList.contains("wiki-standalone-button")) return
  const type = link.dataset.buttonType
  if (type === "set-tags") {
    event.preventDefault()
    actions.setTags(link.dataset.tags ?? "")
  } else if (type === "tags") {
    event.preventDefault()
    actions.openTags()
  }
}
