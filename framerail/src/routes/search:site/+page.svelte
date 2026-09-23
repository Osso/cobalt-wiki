<script lang="ts">
  import type { PageProps } from "./$types"

  let { data }: PageProps = $props()
</script>

<svelte:head>
  <title>Search results</title>
</svelte:head>

<h1>Search results</h1>
<form action="/search:site" method="GET">
  <label for="search-page-query">Search this wiki</label>
  <input id="search-page-query" name="query" type="search" value={data.query} />
  <button type="submit">Search</button>
</form>

{#if data.message}
  <p role="status">{data.message}</p>
{/if}

{#if data.hits.length}
  <ul>
    {#each data.hits as hit (hit.page_id)}
      <li>
        <h2><a href={`/${hit.slug}`}>{hit.title}</a></h2>
        <p>{hit.snippet}</p>
        {#if hit.tags.length}<p>Tags: {hit.tags.join(", ")}</p>{/if}
      </li>
    {/each}
  </ul>
{/if}

<nav aria-label="Search results pages">
  {#if data.previousUrl}<a href={data.previousUrl}>Previous</a>{/if}
  {#if data.nextUrl}<a href={data.nextUrl}>Next</a>{/if}
</nav>
