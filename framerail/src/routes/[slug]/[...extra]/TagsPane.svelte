<script lang="ts">
  import { tagChangesBetween } from "$lib/tag-buttons"
  import { pageLayoutState } from "$lib/stores.svelte"
  import { Layout } from "$lib/types"
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
  let value = $state(
    (pageLayoutState.current === Layout.WIKIDOT ? [...current].sort() : current).join(" ")
  )

  function submit(event: SubmitEvent) {
    event.preventDefault()
    saveTagChanges(tagChangesBetween(current, value.split(/\s+/).filter(Boolean)))
  }
</script>

{#if pageLayoutState.current === Layout.WIKIDOT}
  <h1 class="page-tags-header">Page Tags</h1>
  <p>
    Tags are a nice way to organize content in your Site. You can apply multiple tags
    (labels) to each of your pages. You can learn more what a tag is reading Wikipedia
    entries for <a
      href="http://en.wikipedia.org/wiki/Tags"
      rel="noopener noreferrer"
      target="_blank">Tags</a
    >
    and
    <a
      href="http://en.wikipedia.org/wiki/Tag_cloud"
      rel="noopener noreferrer"
      target="_blank">Tag cloud</a
    >.
  </p>
  <form id="page-tags" onsubmit={submit}>
    <table class="form">
      <tbody>
        <tr>
          <td><label for="page-tags-input">Tags:</label></td>
          <td>
            <input id="page-tags-input" name="tags" class="text" size="50" bind:value />
            <div class="sub">Space-separated list of tags.</div>
          </td>
        </tr>
      </tbody>
    </table>
    <div class="buttons">
      <input class="btn btn-default" onclick={close} type="button" value="close" />
      <input
        class="btn btn-default"
        onclick={() => (value = "")}
        type="button"
        value="clear"
      />
      <input class="btn btn-primary" type="submit" value="save tags" />
    </div>
  </form>
{:else}
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
{/if}

<style lang="scss">
  .page-tags-form {
    display: flex;
    flex-direction: column;
    gap: 15px;
    width: 100%;
    padding: 0 0 2em;
  }
</style>
