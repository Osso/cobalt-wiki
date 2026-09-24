<script lang="ts">
  import { tagChangesBetween } from "$lib/tag-buttons"
  import { untrack } from "svelte"

  import type { PageProps } from "./$types"

  let {
    close,
    saveTagChanges,
    data
  }: PageProps & {
    close: () => void
    saveTagChanges: (changes: string) => void
  } = $props()

  const current = untrack(() => data.page_revision?.tags ?? [])
  let value = $state(current.join(" "))

  function submit(event: SubmitEvent) {
    event.preventDefault()
    saveTagChanges(tagChangesBetween(current, value.split(/\s+/).filter(Boolean)))
  }
</script>

<h1 class="page-tags-header">{data.internationalization?.tags}</h1>

<form id="page-tags" class="page-tags-form" onsubmit={submit}>
  <input name="tags" class="page-tags-input" bind:value />
  <div class="buttons">
    <input
      class="btn btn-danger"
      onclick={close}
      type="button"
      value={data.internationalization?.cancel}
    />
    <input
      class="btn btn-primary"
      type="submit"
      value={data.internationalization?.save}
    />
  </div>
</form>

<style lang="scss">
  .page-tags-form {
    display: flex;
    flex-direction: column;
    gap: 15px;
    width: 100%;
    padding: 0 0 2em;
  }
</style>
