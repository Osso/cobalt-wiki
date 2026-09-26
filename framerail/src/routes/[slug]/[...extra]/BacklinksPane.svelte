<script lang="ts">
  import { deserialize } from "$app/forms"
  import { onMount } from "svelte"
  import type { PageBacklinks } from "$lib/server/load/page-backlinks"

  let backlinks = $state<PageBacklinks | null>(null)
  let message = $state("")

  async function loadBacklinks() {
    try {
      const response = await fetch("?/backlinks", { method: "POST" })
      const result = deserialize<{ res: PageBacklinks }, { message?: string }>(
        await response.text()
      )
      if (result.type !== "success" || !result.data) {
        throw new Error(
          result.type === "failure"
            ? (result.data?.message ?? "Unable to load backlinks")
            : "Unable to load backlinks"
        )
      }
      backlinks = result.data.res
    } catch (cause) {
      message = cause instanceof Error ? cause.message : "Unable to load backlinks"
    }
  }

  onMount(() => {
    void loadBacklinks()
  })
</script>

<section aria-label="Page backlinks">
  <h1>Other pages that depend on this page</h1>
  {#if message}
    <p role="alert">{message}</p>
  {:else if backlinks}
    <h2>Backlinks</h2>
    {#if backlinks.links.length}
      <ul>
        {#each backlinks.links as item (item.page_id)}
          <li><a href={`/${item.slug}`}>{item.title || item.slug} ({item.slug})</a></li>
        {/each}
      </ul>
    {:else}
      <p>No pages directly link to this page.</p>
    {/if}
    <h2>Inclusions</h2>
    {#if backlinks.inclusions.length}
      <ul>
        {#each backlinks.inclusions as item (item.page_id)}
          <li><a href={`/${item.slug}`}>{item.title || item.slug} ({item.slug})</a></li>
        {/each}
      </ul>
    {:else}
      <p>No pages directly include this page.</p>
    {/if}
    {#if backlinks.links.length || backlinks.inclusions.length}
      <p>Title of each page is given and page name (address) in parenthesis.</p>
    {/if}
  {:else}
    <p role="status">Loading backlinks…</p>
  {/if}
</section>
