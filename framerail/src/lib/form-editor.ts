export type FormScalar = string | number | boolean | null
export type FormValues = Record<string, FormScalar>
export interface FormField {
  name: string
  kind: "static" | "text" | "select" | "wiki"
  properties: Record<string, unknown>
  options: { code: FormScalar; label: FormScalar }[]
}
export interface PageForm {
  schema: { fields: FormField[]; properties: Record<string, unknown> }
  values: FormValues
}
export type FormDraft = Record<string, FormScalar | undefined>

export function scalarText(value: unknown): string {
  return typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
    ? String(value)
    : ""
}

function initialValue(field: FormField, values: FormValues): FormScalar | undefined {
  if (Object.hasOwn(values, field.name)) return values[field.name]
  const value = field.properties.default
  if (value === null || ["string", "number", "boolean"].includes(typeof value)) {
    return value as FormScalar
  }
  return undefined
}

export function fieldText(field: FormField, values: FormValues): string {
  return scalarText(field.properties.value ?? initialValue(field, values))
}

export function createDraft(form: PageForm): FormDraft {
  return Object.fromEntries(
    form.schema.fields
      .filter((field) => field.kind !== "static")
      .map((field) => {
        const value = initialValue(field, form.values)
        return [field.name, field.kind === "select" ? value : scalarText(value)]
      })
  )
}

export function changedFields(form: PageForm, draft: FormDraft): FormValues {
  const initial = createDraft(form)
  return Object.fromEntries(
    form.schema.fields
      .filter((field) => field.kind !== "static")
      .filter(
        (field) =>
          draft[field.name] !== undefined && draft[field.name] !== initial[field.name]
      )
      .map((field) => [field.name, draft[field.name] as FormScalar])
  )
}

export function editContent(
  wikitext: string | undefined,
  updates: FormValues | undefined
) {
  if ((wikitext === undefined) === (updates === undefined)) {
    throw new Error("Page edit requires exactly one of wikitext or form updates")
  }
  return updates === undefined ? { wikitext } : { form_updates: updates }
}
