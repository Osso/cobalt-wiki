export function preventEditorImplicitSubmit(form: HTMLFormElement) {
  function onKeydown(event: KeyboardEvent): void {
    const target = event.target
    if (
      event.key === "Enter" &&
      !event.isComposing &&
      target instanceof HTMLInputElement &&
      (target.type === "text" || target.type === "radio") &&
      target.form === form
    ) {
      event.preventDefault()
    }
  }

  form.addEventListener("keydown", onKeydown)
  return {
    destroy() {
      form.removeEventListener("keydown", onKeydown)
    }
  }
}
