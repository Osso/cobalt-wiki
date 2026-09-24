export function preventEditorImplicitSubmit(event: KeyboardEvent): void {
  const target = event.target
  if (
    event.key === "Enter" &&
    !event.isComposing &&
    target instanceof HTMLInputElement &&
    (target.type === "text" || target.type === "radio") &&
    target.form === event.currentTarget
  ) {
    event.preventDefault()
  }
}
