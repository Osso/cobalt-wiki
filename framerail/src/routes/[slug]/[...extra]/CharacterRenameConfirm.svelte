<script lang="ts">
  import type { CharacterRenamePreview } from "$lib/server/deepwell/character-rename"

  let {
    preview,
    busy,
    confirm,
    cancel
  }: {
    preview: CharacterRenamePreview
    busy: boolean
    confirm: () => void
    cancel: () => void
  } = $props()
</script>

<section aria-label="Rename preview">
  <p><strong>{preview.old_slug} → {preview.new_slug}</strong></p>
  <p>
    {preview.count}
    {preview.count === 1 ? "page" : "pages"} will be edited:
  </p>
  <ul>
    {#each preview.pages as page (page.page_id)}
      <li>
        <a href={`/${encodeURIComponent(page.slug)}`}>{page.title || page.slug}</a>
        ({page.slug})
      </li>
    {/each}
  </ul>
  <p>
    {preview.icon ? "The icon file will be renamed." : "No icon file will be renamed."}
  </p>
  <div class="buttons">
    <input
      class="btn btn-danger"
      disabled={busy}
      onclick={cancel}
      type="button"
      value="Cancel"
    />
    <input
      class="btn btn-primary"
      disabled={busy}
      onclick={confirm}
      type="button"
      value="Confirm rename"
    />
  </div>
</section>

<style lang="scss">
  ul {
    padding-left: 1.5em;
  }
</style>
