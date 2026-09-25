<script lang="ts">
  import { resolve } from "$app/paths"
  import type { PageProps } from "./$types"
  let { data }: PageProps = $props()
</script>

<h1>Activity</h1>
{#if data.change}
  <article>
    <h2>{data.change.title}</h2>
    <p>
      {data.change.event_type === "create" ? "Created" : "Edited"} by {data.change.actor}
      <time datetime={data.change.created_at}>{data.change.created_at}</time>
    </p>
    {#if data.change.before_text !== null}
      <h3>Before</h3>
      <pre>{data.change.before_text}</pre>
    {/if}
    <h3>After</h3>
    <pre>{data.change.after_text}</pre>
  </article>
  <p><a href={resolve("/-/activity", {})}>Back to Activity</a></p>
{:else if data.items.length === 0}
  <p>No watched page changes here.</p>
{:else}
  <ul>
    {#each data.items as item (item.event_id)}
      <li>
        <a href={`${resolve("/-/activity", {})}?event=${item.event_id}`}>{item.title}</a>
        — {item.event_type === "create" ? "created" : "edited"} by {item.actor}
        <time datetime={item.created_at}>{item.created_at}</time>
      </li>
    {/each}
  </ul>
{/if}
{#if data.next_before !== null}
  <a href={`?before=${data.next_before}`}>Older activity</a>
{/if}
