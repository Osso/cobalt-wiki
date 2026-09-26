<script lang="ts">
  import type { BacklinkPage, PageBacklinks } from "$lib/server/load/page-backlinks"

  let {
    dependencies,
    selectedIds = $bindable(),
    remaining = false
  }: {
    dependencies: PageBacklinks
    selectedIds: number[]
    remaining?: boolean
  } = $props()

  const sources = $derived([
    ...new Map(
      [...dependencies.links, ...dependencies.inclusions].map((page) => [
        page.page_id,
        page
      ])
    ).values()
  ])

  function sourceLink(page: BacklinkPage) {
    return `/${encodeURIComponent(page.slug)}`
  }
</script>

{#if remaining}
  <section aria-label="Dependencies remaining after move">
    <h2>Links</h2>
    {#if dependencies.links.length}
      <ul>
        {#each dependencies.links as page (page.page_id)}
          <li><a href={sourceLink(page)}>{page.title || page.slug} ({page.slug})</a></li>
        {/each}
      </ul>
    {:else}
      <p>No links remain.</p>
    {/if}
    <h2>Inclusions</h2>
    {#if dependencies.inclusions.length}
      <ul>
        {#each dependencies.inclusions as page (page.page_id)}
          <li><a href={sourceLink(page)}>{page.title || page.slug} ({page.slug})</a></li>
        {/each}
      </ul>
    {:else}
      <p>No inclusions remain.</p>
    {/if}
    <p>
      These pages were not repaired. Manually edit them if their links or inclusions need
      updating.
    </p>
  </section>
{:else}
  <section aria-label="Dependency repair choices">
    {#if sources.length}
      <p>
        Select pages whose links or inclusions you want to repair. You need Edit
        permission on each selected page. Unselected pages will not be repaired.
      </p>
      <ul>
        {#each sources as page (page.page_id)}
          <li>
            <label>
              <input type="checkbox" value={page.page_id} bind:group={selectedIds} />
              <a href={sourceLink(page)}>{page.title || page.slug} ({page.slug})</a>
            </label>
          </li>
        {/each}
      </ul>
      <div class="dependency-actions">
        <button
          onclick={() => (selectedIds = sources.map((page) => page.page_id))}
          type="button">Select all</button
        >
        <button onclick={() => (selectedIds = [])} type="button">Unselect all</button>
      </div>
    {:else}
      <p>No pages directly link to or include this page.</p>
    {/if}
  </section>
{/if}

<style lang="scss">
  ul {
    padding-left: 1.5em;
  }

  li {
    margin: 0.5em 0;
  }

  input[type="checkbox"] {
    margin-right: 0.5em;
  }

  .dependency-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 1em;
  }
</style>
