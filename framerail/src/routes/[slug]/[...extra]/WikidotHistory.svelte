<script lang="ts">
  import { deserialize } from "$app/forms"
  import { onMount } from "svelte"
  import HistoryTable from "./HistoryTable.svelte"
  import HistoryDetail from "./HistoryDetail.svelte"

  import type {
    HistoryComparison,
    HistoryFilters,
    HistoryList,
    HistoryOrigin,
    HistoryRevision,
    HistoryRow
  } from "$lib/server/load/history"

  const filterLabels: { key: keyof HistoryFilters; label: string }[] = [
    { key: "all", label: "ALL" },
    { key: "source", label: "source changes" },
    { key: "title", label: "title changes" },
    { key: "move", label: "page name changes" },
    { key: "tags", label: "tags changes" },
    { key: "meta", label: "metadata changes" },
    { key: "files", label: "files changes" }
  ]
  let filters = $state<HistoryFilters>({
    all: true,
    source: false,
    title: false,
    move: false,
    tags: false,
    meta: false,
    files: false
  })
  let origin = $state<HistoryOrigin>("wikidot")
  let page = $state(1)
  let perPage = $state(20)
  let listing = $state<HistoryList | null>(null)
  let revision = $state<HistoryRevision | null>(null)
  let comparison = $state<HistoryComparison | null>(null)
  let rendered = $state(false)
  let from = $state<number | null>(null)
  let to = $state<number | null>(null)
  let busy = $state(false)
  let message = $state("")

  async function postHistory<T>(
    action: string,
    input: Record<string, unknown>
  ): Promise<T> {
    const response = await fetch(`?/${action}`, {
      method: "POST",
      body: JSON.stringify(input)
    })
    const result = deserialize<{ res: T }, { message?: string }>(await response.text())
    if (result.type !== "success" || !result.data) {
      throw new Error(
        result.type === "failure"
          ? (result.data?.message ?? "History request failed")
          : "History request failed"
      )
    }
    return result.data.res
  }

  function reportFailure(cause: unknown) {
    message = cause instanceof Error ? cause.message : "History request failed"
  }

  async function readHistoryListing(
    nextPage: number,
    nextOrigin: HistoryOrigin
  ): Promise<HistoryList> {
    const result = await postHistory<HistoryList>("historyList", {
      origin: nextOrigin,
      page: nextPage,
      per_page: perPage,
      filters
    })
    if (nextOrigin === "wikidot" && !result.available.wikidot && result.available.local) {
      return postHistory<HistoryList>("historyList", {
        origin: "local",
        page: 1,
        per_page: perPage,
        filters
      })
    }
    return result
  }

  async function loadList(nextPage = page, nextOrigin = origin) {
    if (busy) return
    busy = true
    message = ""
    revision = null
    comparison = null
    try {
      listing = await readHistoryListing(nextPage, nextOrigin)
      origin = listing.origin
      page = listing.page
      to = listing.rows[0]?.number ?? null
      from = listing.rows[1]?.number ?? null
    } catch (cause) {
      listing = null
      reportFailure(cause)
    } finally {
      busy = false
    }
  }

  function updateFilter(key: keyof HistoryFilters, checked: boolean) {
    filters = { ...filters, [key]: checked }
    page = 1
  }

  function selectOrigin(value: HistoryOrigin) {
    origin = value
    page = 1
    void loadList(1, value)
  }

  async function openRevision(row: HistoryRow, showRendered: boolean) {
    if (busy) return
    busy = true
    message = ""
    revision = null
    comparison = null
    try {
      const result = await postHistory<HistoryRevision | null>("historyRevision", {
        origin,
        number: row.number,
        rendered: showRendered
      })
      if (!result) throw new Error(`Revision ${row.number} was not found`)
      revision = result
      rendered = showRendered
    } catch (cause) {
      reportFailure(cause)
    } finally {
      busy = false
    }
  }

  async function compareVersions() {
    if (busy || from === null || to === null || from === to) return
    busy = true
    message = ""
    revision = null
    comparison = null
    try {
      comparison = await postHistory<HistoryComparison>("historyCompare", {
        origin,
        from: Math.min(from, to),
        to: Math.max(from, to)
      })
    } catch (cause) {
      reportFailure(cause)
    } finally {
      busy = false
    }
  }

  onMount(() => {
    void loadList()
  })
</script>

<section aria-busy={busy} aria-label="Page history">
  <h1>Page history of changes</h1>
  <div class="history-controls">
    {#if listing && listing.available.wikidot && listing.available.local}
      <label for="history-dataset">History dataset</label>
      <select
        id="history-dataset"
        disabled={busy}
        onchange={(event) => selectOrigin(event.currentTarget.value as HistoryOrigin)}
        value={origin}
      >
        <option value="wikidot">Wikidot source</option>
        <option value="local">Local revisions</option>
      </select>
    {:else}
      <span
        >History dataset: {origin === "wikidot"
          ? "Wikidot source"
          : "Local revisions"}</span
      >
    {/if}
    <fieldset>
      <legend>Show page changes</legend>
      {#each filterLabels as filter (filter.key)}
        <label
          ><input
            checked={filters[filter.key]}
            onchange={(event) => updateFilter(filter.key, event.currentTarget.checked)}
            type="checkbox"
          />{filter.label}</label
        >
      {/each}
    </fieldset>
    <div class="history-page-size">
      <label for="history-perpage">Revisions per page:</label>
      <select
        id="history-perpage"
        onchange={() => {
          page = 1
        }}
        bind:value={perPage}
      >
        <option value={10}>10</option><option value={20}>20</option><option value={50}
          >50</option
        >
        <option value={100}>100</option><option value={200}>200</option>
      </select>
    </div>
    <div class="history-buttons">
      <button disabled={busy} onclick={() => loadList(1)} type="button"
        >Update list</button
      >
      <button
        disabled={busy || from === null || to === null || from === to}
        onclick={compareVersions}
        type="button">Compare versions</button
      >
    </div>
  </div>
  {#if message}<p role="alert">{message}</p>{/if}
  {#if busy}<p role="status">Loading history…</p>{/if}
  {#if listing}
    <HistoryTable
      {busy}
      changePage={(number) => loadList(number)}
      {from}
      {listing}
      {openRevision}
      selectFrom={(number) => {
        from = number
      }}
      selectTo={(number) => {
        to = number
      }}
      {to}
    />
  {/if}
  <HistoryDetail {comparison} {origin} {rendered} {revision} />
</section>

<style>
  .history-controls {
    width: fit-content;
    max-width: 100%;
    margin: 1rem auto;
  }
  fieldset {
    display: grid;
    grid-template-columns: minmax(10rem, auto) auto;
    row-gap: 0.1rem;
    column-gap: 0.75rem;
    padding: 0;
    margin-block: 0.75rem;
    border: 0;
  }
  fieldset legend {
    float: left;
    width: 10rem;
  }
  fieldset label {
    display: block;
    grid-column: 2;
    margin: 0;
    line-height: 1.35;
  }
  fieldset input {
    margin-right: 0.4rem;
  }
  .history-page-size {
    display: grid;
    grid-template-columns: minmax(10rem, auto) auto;
    gap: 0.75rem;
    align-items: center;
  }
  .history-page-size select {
    justify-self: start;
    width: auto;
  }
  .history-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    justify-content: center;
    margin-block: 0.75rem;
  }
</style>
