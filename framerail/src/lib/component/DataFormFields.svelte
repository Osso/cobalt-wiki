<script lang="ts">
  import { scalarText, fieldText } from "../form-editor"
  import type { PageForm, FormDraft } from "../form-editor"

  let { form, draft = $bindable() }: { form: PageForm; draft: FormDraft } = $props()

  function dimension(value: unknown): number | undefined {
    const number = Number(value)
    return Number.isInteger(number) && number > 0 ? number : undefined
  }
</script>

{#each form.schema.fields as field, index (field.name)}
  {@const id = `data-form-field-${index}`}
  {@const hint = scalarText(field.properties.hint ?? field.properties.Hint)}
  <div class="form-field">
    {#if field.kind === "static"}
      <div class="static-field">{fieldText(field, form.values)}</div>
    {:else}
      <label for={id}>{scalarText(field.properties.label) || field.name}</label>
      {#if field.kind === "select"}
        <select
          {id}
          aria-describedby={hint ? `${id}-hint` : undefined}
          bind:value={draft[field.name]}
        >
          {#if !field.options.some((option) => option.code === draft[field.name])}
            <option value={draft[field.name]}>{scalarText(draft[field.name])}</option>
          {/if}
          {#each field.options as option, optionIndex (optionIndex)}
            <option value={option.code}>{scalarText(option.label)}</option>
          {/each}
        </select>
      {:else if field.kind === "wiki"}
        <textarea
          {id}
          aria-describedby={hint ? `${id}-hint` : undefined}
          cols={dimension(field.properties.width)}
          oninput={(event) => (draft[field.name] = event.currentTarget.value)}
          rows={dimension(field.properties.height)}
          value={scalarText(draft[field.name])}></textarea>
      {:else}
        <input
          {id}
          aria-describedby={hint ? `${id}-hint` : undefined}
          oninput={(event) => (draft[field.name] = event.currentTarget.value)}
          size={dimension(field.properties.width)}
          type="text"
          value={scalarText(draft[field.name])}
        />
      {/if}
    {/if}
    {#if hint}<small id={`${id}-hint`}>{hint}</small>{/if}
  </div>
{/each}

<style>
  .form-field {
    display: flex;
    flex-direction: column;
    gap: 0.25em;
  }
  input,
  textarea,
  select {
    box-sizing: border-box;
    max-width: 100%;
  }
  .static-field {
    white-space: pre-wrap;
  }
</style>
