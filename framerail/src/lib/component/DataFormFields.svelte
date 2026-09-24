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
  {@const label = scalarText(field.properties.label)}
  {@const hint = scalarText(field.properties.hint)}
  {@const after = scalarText(field.properties.after)}
  <div class="form-field">
    {#if field.kind === "static"}
      {#if scalarText(field.properties.label)}
        <div class="static-label">{scalarText(field.properties.label)}</div>
      {/if}
      <div class="static-field">{fieldText(field, form.values)}</div>
    {:else if field.kind === "select" && field.options.length >= 2 && field.options.length <= 4}
      <div class="field-control">
        <fieldset
          aria-describedby={after ? `${id}-after` : undefined}
          aria-label={label ? undefined : field.name}
        >
          {#if label}<legend>{label}</legend>{/if}
          {#if draft[field.name] !== undefined && !field.options.some((option) => option.code === draft[field.name])}
            <output>Current value: {String(draft[field.name])}</output>
          {/if}
          {#each field.options as option, optionIndex (optionIndex)}
            <label class="radio-option" for={`${id}-${optionIndex}`}>
              <input
                id={`${id}-${optionIndex}`}
                name={id}
                type="radio"
                value={option.code}
                bind:group={draft[field.name]}
              />
              {scalarText(option.label)}
            </label>
          {/each}
        </fieldset>
        {#if after}<small id={`${id}-after`}>{after}</small>{/if}
      </div>
    {:else}
      {#if label}<label for={id}>{label}</label>{/if}
      <div class="field-control">
        {#if field.kind === "select"}
          <select
            {id}
            aria-describedby={after ? `${id}-after` : undefined}
            aria-label={label ? undefined : field.name}
            bind:value={draft[field.name]}
          >
            {#if !field.options.some((option) => option.code === draft[field.name])}
              <option value={draft[field.name]}>{scalarText(draft[field.name])}</option>
            {/if}
            {#each field.options as option, optionIndex (optionIndex)}
              <option value={option.code}>{scalarText(option.label)}</option>
            {/each}
          </select>
        {:else if field.kind === "wiki" || (dimension(field.properties.height) ?? 0) >= 2}
          <textarea
            {id}
            aria-describedby={after ? `${id}-after` : undefined}
            aria-label={label ? undefined : field.name}
            cols={dimension(field.properties.width)}
            oninput={(event) => (draft[field.name] = event.currentTarget.value)}
            placeholder={hint || undefined}
            rows={dimension(field.properties.height)}
            value={scalarText(draft[field.name])}></textarea>
        {:else}
          <input
            {id}
            aria-describedby={after ? `${id}-after` : undefined}
            aria-label={label ? undefined : field.name}
            oninput={(event) => (draft[field.name] = event.currentTarget.value)}
            placeholder={hint || undefined}
            size={dimension(field.properties.width)}
            type="text"
            value={scalarText(draft[field.name])}
          />
        {/if}
        {#if after}<small id={`${id}-after`}>{after}</small>{/if}
      </div>
    {/if}
  </div>
{/each}

<style>
  .form-field {
    display: flex;
    flex-direction: column;
    gap: 0.25em;
  }
  .field-control {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25em;
    align-items: baseline;
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
  fieldset {
    margin: 0;
    border: 0;
    padding: 0;
  }
  legend {
    padding: 0;
  }
  .radio-option {
    display: flex;
    align-items: center;
    gap: 0.4em;
  }
</style>
