export type KeyboardEventKey = {
  key: string
  ctrlKey: boolean
  altKey: boolean
  metaKey: boolean
  shiftKey: boolean
  isComposing?: boolean
}

export type KeyboardControl = "bold" | "italic" | "underline" | "tab" | null

export function shortcutControl(event: KeyboardEventKey): KeyboardControl {
  if (event.isComposing) return null
  if (event.key === "Tab") return "tab"
  if (!event.ctrlKey || event.altKey) return null
  if (event.key === "b") return "bold"
  if (event.key === "i") return "italic"
  if (event.key === "u") return "underline"
  return null
}

type Replacement = readonly [RegExp, string]

const listRules: Replacement[] = [
  [/((?:\r?\n|^)\s*[\*#:]\s.*?\r?\n)\s*[\*#:]\s\r?\n$/, "$1\n"],
  [/((?:\r?\n|^)([\*#])\s.*?\r?\n)$/, "$1$2 "],
  [/(\r?\n *[\*#]\s.+\r?\n( *)([\*#])\s.*?\r?\n)$/, "$1$2$3 "]
]

const remainingRules: Replacement[] = [
  [/(\r?\n:\s.+?\s:.*\r?\n)$/, "$1: "],
  [/(\r?\n(\t+).+\r?\n)$/, "$1$2"],
  [/(\r?\n(\t+)\r?\n)$/, "\n\n"]
]

const blockOpening = /(\[\[(code|embedvideo|math|embed)(?:\s[^\]]*?)?\]\]\r?\n)$/

function applyRules(text: string, rules: Replacement[]): string {
  return rules.reduce(
    (current, [pattern, replacement]) => current.replace(pattern, replacement),
    text
  )
}

export function applyEnterAssist(
  value: string,
  caret: number
): { value: string; start: number; end: number } {
  if (!Number.isInteger(caret) || caret < 0 || caret > value.length) {
    throw new RangeError("Invalid Enter assistance caret")
  }

  const beforeBlock = applyRules(value.slice(0, caret), listRules)
  const completed = beforeBlock.replace(blockOpening, "$1\n[[/$2]]")
  const beforeCaret = applyRules(completed.slice(0, beforeBlock.length), remainingRules)
  const afterCaret = completed.slice(beforeBlock.length) + value.slice(caret)
  return {
    value: beforeCaret + afterCaret,
    start: beforeCaret.length,
    end: beforeCaret.length
  }
}
