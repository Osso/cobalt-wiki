/** FTML tabs arrive as compiled HTML, not Svelte components. */
function findTabButton(target: EventTarget | null) {
  return target instanceof Element
    ? target.closest<HTMLElement>("wj-tabs-button.wj-tabs-button")
    : null
}

function readTabButtons(tabview: Element) {
  return Array.from(
    tabview.querySelectorAll<HTMLElement>(
      ":scope > .wj-tabs-button-list > .wj-tabs-button"
    )
  )
}

function selectTab(button: HTMLElement) {
  const tabview = button.closest("wj-tabs.wj-tabs")
  if (!tabview) return
  const buttons = readTabButtons(tabview)
  const panels = tabview.querySelectorAll<HTMLElement>(
    ":scope > .wj-tabs-panel-list > .wj-tabs-panel"
  )
  const selected = buttons.indexOf(button)
  tabview.setAttribute("panel-selected", String(selected))
  buttons.forEach((tab, index) => {
    const active = index === selected
    tab.setAttribute("aria-selected", String(active))
    tab.tabIndex = active ? 0 : -1
  })
  panels.forEach((panel, index) => {
    panel.hidden = index !== selected
  })
}

export function clickTabview(event: MouseEvent) {
  const button = findTabButton(event.target)
  if (!button) return
  event.preventDefault()
  selectTab(button)
}

/** Arrow keys move focus; Enter and Space activate the focused tab. */
export function keydownTabview(event: KeyboardEvent) {
  const button = findTabButton(event.target)
  if (!button) return
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault()
    selectTab(button)
    return
  }
  const tabview = button.closest("wj-tabs.wj-tabs")
  if (!tabview) return
  const buttons = readTabButtons(tabview)
  const index = buttons.indexOf(button)
  let next: HTMLElement | undefined
  switch (event.key) {
    case "ArrowRight":
      next = buttons[(index + 1) % buttons.length]
      break
    case "ArrowLeft":
      next = buttons[(index + buttons.length - 1) % buttons.length]
      break
    case "Home":
      next = buttons[0]
      break
    case "End":
      next = buttons[buttons.length - 1]
      break
  }
  if (next) {
    event.preventDefault()
    next.focus()
  }
}
