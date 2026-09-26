<script lang="ts">
  import type {
    HistoryComparison,
    HistoryOrigin,
    HistoryRevision
  } from "$lib/server/load/history"

  let {
    origin,
    revision,
    comparison,
    rendered
  }: {
    origin: HistoryOrigin
    revision: HistoryRevision | null
    comparison: HistoryComparison | null
    rendered: boolean
  } = $props()
</script>

{#if origin === "wikidot" && (revision || comparison)}
  <p>Captured Wikidot source; historical whitespace may differ.</p>
{/if}
{#if revision}
  <section aria-label={`Revision ${revision.number} detail`}>
    <h2>Revision {revision.number} {rendered ? "view" : "source"}</h2>
    {#if revision.representation}<p>Representation: {revision.representation}</p>{/if}
    {#if rendered}
      <p>
        Read-only preview using current rendering context; not an original compiled
        snapshot.
      </p>
      <div class="history-preview" aria-label="Read-only preview">
        {@html revision.rendered_html ?? ""}
      </div>
    {:else}
      <label for="history-source">Revision source</label>
      <textarea id="history-source" readonly value={revision.source}></textarea>
    {/if}
  </section>
{/if}
{#if comparison}
  <section aria-label="Revision comparison">
    <h2>Compare revisions {comparison.from} to {comparison.to}</h2>
    {#if comparison.representation}<p>Representation: {comparison.representation}</p>{/if}
    <pre class="history-diff">{#each comparison.lines as line, index (index)}<span
          class={line.kind}
          >{#if line.kind === "insert"}<ins>{line.text}</ins
            >{:else if line.kind === "delete"}<del>{line.text}</del
            >{:else}{line.text}{/if}</span
        >{/each}</pre>
  </section>
{/if}

<style>
  textarea {
    box-sizing: border-box;
    width: 100%;
    min-height: 20rem;
    font-family: monospace;
  }
  .history-preview {
    padding: 0.75rem;
    border: 1px solid currentColor;
  }
  .history-diff {
    overflow-x: auto;
    white-space: pre-wrap;
  }
  .history-diff span {
    display: block;
  }
  .insert {
    background: #e6f5e6;
  }
  .delete {
    background: #f9e7e7;
  }
</style>
