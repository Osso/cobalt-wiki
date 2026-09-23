<script lang="ts">
  import { deserialize } from "$app/forms"
  import { onMount } from "svelte"

  import type {
    ImportedRevisionSource,
    ImportedRevisionSummary
  } from "$lib/server/deepwell/page"

  const pageSize = 50
  let revisions = $state<ImportedRevisionSummary[]>([])
  let selected = $state<ImportedRevisionSource | null>(null)
  let loading = $state(false)
  let hasOlder = $state(true)
  let error = $state("")

  async function requestHistory<T>(action: string, parameters: Record<string, number>) {
    const response = await fetch(`?/${action}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(parameters)
    })
    const result = deserialize<{ res: T }, { message?: string }>(await response.text())
    if (result.type !== "success" || !result.data) {
      throw new Error(
        result.type === "failure" && result.data?.message
          ? result.data.message
          : "Could not load imported history."
      )
    }
    return result.data.res
  }

  async function loadOlder() {
    if (loading) return
    loading = true
    error = ""
    try {
      const beforeRevision = revisions.at(-1)?.source_revision_number
      const parameters: Record<string, number> =
        beforeRevision === undefined
          ? { limit: pageSize }
          : { limit: pageSize, beforeRevision }
      const entries = await requestHistory<ImportedRevisionSummary[]>(
        "importedHistory",
        parameters
      )
      revisions = [...revisions, ...entries]
      hasOlder = entries.length === pageSize
    } catch (failure) {
      error =
        failure instanceof Error ? failure.message : "Could not load imported history."
    } finally {
      loading = false
    }
  }

  async function viewSource(sourceRevisionNumber: number) {
    if (loading) return
    loading = true
    error = ""
    selected = null
    try {
      selected = await requestHistory<ImportedRevisionSource | null>("importedRevision", {
        sourceRevisionNumber
      })
      if (selected === null) throw new Error("Imported source revision was not found.")
    } catch (failure) {
      error =
        failure instanceof Error ? failure.message : "Could not load historical source."
    } finally {
      loading = false
    }
  }

  onMount(loadOlder)
</script>

<section aria-labelledby="imported-history-heading" aria-busy={loading}>
  <h2 id="imported-history-heading">Imported Wikidot history</h2>
  <p>
    Source records are separate from editable local revisions and cannot be rolled back
    here.
  </p>
  {#if error}
    <p role="alert">{error}</p>
  {/if}
  {#if revisions.length > 0}
    <div class="imported-history-table">
      <table class="page-history">
        <thead>
          <tr>
            <th scope="col">Revision</th>
            <th scope="col">Source</th>
            <th scope="col">Changes</th>
            <th scope="col">Source author</th>
            <th scope="col">Created</th>
            <th scope="col">Comments</th>
          </tr>
        </thead>
        <tbody>
          {#each revisions as revision (revision.source_revision_id)}
            <tr>
              <td>{revision.source_revision_number}</td>
              <td>
                <button
                  type="button"
                  disabled={loading}
                  aria-label={`View source revision ${revision.source_revision_number}`}
                  onclick={() => viewSource(revision.source_revision_number)}
                  >View source</button
                >
                <small>ID {revision.source_revision_id}</small>
              </td>
              <td>{revision.source_flags.join(", ")}</td>
              <td
                >{revision.source_author_id === null
                  ? "Unknown"
                  : `Wikidot ID ${revision.source_author_id}`}</td
              >
              <td
                ><time datetime={revision.source_created_at}
                  >{new Date(revision.source_created_at).toLocaleString()}</time
                ></td
              >
              <td>{revision.source_comments}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else if !loading && !error}
    <p>No imported source revisions.</p>
  {/if}
  {#if loading}
    <p role="status">Loading history…</p>
  {/if}
  {#if hasOlder}
    <button type="button" disabled={loading} onclick={loadOlder}
      >Load older revisions</button
    >
  {/if}
  {#if selected}
    <h3>Source revision {selected.metadata.source_revision_number}</h3>
    <p>Representation: {selected.metadata.representation}</p>
    <p>
      Historical whitespace may differ from the original. Current page content is
      unchanged.
    </p>
    <label for="imported-revision-source">Imported revision source</label>
    <textarea id="imported-revision-source" readonly value={selected.wikitext}></textarea>
  {/if}
</section>

<style>
  section {
    margin-block: 1.5rem;
  }
  .imported-history-table {
    overflow-x: auto;
  }
  th,
  td {
    padding: 0.4rem;
    text-align: left;
    vertical-align: top;
  }
  small {
    display: block;
  }
  textarea {
    box-sizing: border-box;
    width: 100%;
    min-height: 20rem;
    font-family: monospace;
  }
</style>
