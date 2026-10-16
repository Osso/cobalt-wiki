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
  {#if preview.destination_tag_pages > 0}
    <p>
      {preview.destination_tag_pages}
      {preview.destination_tag_pages === 1 ? "page already uses" : "pages already use"}
      <strong>{preview.new_name}</strong> or <strong>re:{preview.new_name}</strong>.
      Existing tags will be kept. Renamed references will join these tags, with duplicates
      removed. Confirm only if these tags should refer to this character.
    </p>
  {/if}
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
      value={preview.destination_tag_pages > 0
        ? "Confirm rename and keep existing tags"
        : "Confirm rename"}
    />
  </div>
</section>

<style lang="scss">
  ul {
    padding-left: 1.5em;
  }
</style>
