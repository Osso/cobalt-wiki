<script lang="ts">
  import { scalarText, fieldText } from "../form-editor"
  import type { PageForm, FormDraft } from "../form-editor"

  let {
    form,
    draft = $bindable(),
    title = $bindable(),
    titleLabel
  }: {
    form: PageForm
    draft: FormDraft
    title: string
    titleLabel: string | undefined
  } = $props()

  function dimension(value: unknown): number | undefined {
    const number = Number(value)
    return Number.isInteger(number) && number > 0 ? number : undefined
  }
</script>

<table class="form-table">
  <tbody>
    <tr>
      <td class="form-labels"><label for="data-form-title">{titleLabel}</label></td>
      <td class="form-values">
        <input
          id="data-form-title"
          class="text"
          name="title"
          size="35"
          type="text"
          bind:value={title}
        />
      </td>
    </tr>
    {#each form.schema.fields as field, index (field.name)}
      {@const id = `data-form-field-${index}`}
      {@const label = scalarText(field.properties.label)}
      {@const hint = scalarText(field.properties.hint)}
      {@const after = scalarText(field.properties.after)}
      <tr class="form-field">
        <td class="form-labels">
          {#if field.kind === "static"}
            {#if label}<div class="static-label">{label}</div>{/if}
          {:else if field.kind === "select" && field.options.length >= 2 && field.options.length <= 4}
            {#if label}<span id={`${id}-label`}>{label}</span>{/if}
          {:else}
            {#if label}<label for={id}>{label}</label>{/if}
          {/if}
        </td>
        <td class="form-values">
          {#if field.kind === "static"}
            <div class="static-field">{fieldText(field, form.values)}</div>
          {:else if field.kind === "select" && field.options.length >= 2 && field.options.length <= 4}
            <fieldset
              aria-describedby={after ? `${id}-after` : undefined}
              aria-label={label ? undefined : field.name}
              aria-labelledby={label ? `${id}-label` : undefined}
            >
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
                  />{scalarText(option.label)}
                </label>
              {/each}
            </fieldset>
            {#if after}<small id={`${id}-after`}>{after}</small>{/if}
          {:else}
            {#if field.kind === "select"}
              <select
                {id}
                aria-describedby={after ? `${id}-after` : undefined}
                aria-label={label ? undefined : field.name}
                bind:value={draft[field.name]}
              >
                {#if !field.options.some((option) => option.code === draft[field.name])}
                  <option value={draft[field.name]}
                    >{scalarText(draft[field.name])}</option
                  >
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
          {/if}
        </td>
      </tr>
    {/each}
  </tbody>
</table>

<style>
  .form-table {
    width: 100%;
    font-family: "Noto Serif", "Times New Roman", Times, serif;
    font-size: 16px;
    font-weight: 400;
    line-height: normal;
  }
  .form-labels,
  .form-values {
    padding: 1px;
    vertical-align: middle;
  }
  .form-labels {
    max-width: 40%;
  }
  #data-form-title {
    font-size: 130%;
    font-weight: bold;
  }
  input,
  textarea,
  select {
    box-sizing: border-box;
    max-width: 100%;
  }
  .static-field {
    margin-bottom: 0.75em;
    white-space: pre-wrap;
  }
  fieldset {
    padding: 0;
    margin: 0;
    border: 0;
  }
  .radio-option {
    display: inline;
  }
</style>
