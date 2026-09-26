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
    { key: "title", label: "title" },
    { key: "move", label: "move/rename" },
    { key: "meta", label: "meta data" },
    { key: "files", label: "attachments (files)" }
  ]
  let filters = $state<HistoryFilters>({
    all: true,
    source: false,
    title: false,
    move: false,
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

  async function loadList(nextPage = page, nextOrigin = origin) {
    if (busy) return
    busy = true
    message = ""
    revision = null
    comparison = null
    try {
      const result = await postHistory<HistoryList>("historyList", {
        origin: nextOrigin,
        page: nextPage,
        per_page: perPage,
        filters
      })
      if (
        nextOrigin === "wikidot" &&
        result.available.wikidot === 0 &&
        result.available.local > 0
      ) {
        listing = await postHistory<HistoryList>("historyList", {
          origin: "local",
          page: 1,
          per_page: perPage,
          filters
        })
      } else {
        listing = result
      }
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
  <div class="history-controls">
    {#if listing && listing.available.wikidot > 0 && listing.available.local > 0}
      <label for="history-dataset">History dataset</label>
      <select
        id="history-dataset"
        value={origin}
        onchange={(event) => selectOrigin(event.currentTarget.value as HistoryOrigin)}
        disabled={busy}
      >
        <option value="wikidot">Wikidot source ({listing.available.wikidot})</option>
        <option value="local">Local revisions ({listing.available.local})</option>
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
      {#each filterLabels as filter}
        <label
          ><input
            type="checkbox"
            checked={filters[filter.key]}
            onchange={(event) => updateFilter(filter.key, event.currentTarget.checked)}
          />{filter.label}</label
        >
      {/each}
    </fieldset>
    <label for="history-perpage">Revisions per page:</label>
    <select
      id="history-perpage"
      bind:value={perPage}
      onchange={() => {
        page = 1
      }}
    >
      <option value={10}>10</option><option value={20}>20</option><option value={50}
        >50</option
      >
      <option value={100}>100</option><option value={200}>200</option>
    </select>
    <div class="history-buttons">
      <button type="button" disabled={busy} onclick={() => loadList(1)}
        >update list</button
      >
      <button
        type="button"
        disabled={busy || from === null || to === null || from === to}
        onclick={compareVersions}>compare versions</button
      >
    </div>
  </div>
  {#if message}<p role="alert">{message}</p>{/if}
  {#if busy}<p role="status">Loading history…</p>{/if}
  {#if listing}
    <HistoryTable
      {listing}
      {from}
      {to}
      {busy}
      selectFrom={(number) => {
        from = number
      }}
      selectTo={(number) => {
        to = number
      }}
      changePage={(number) => loadList(number)}
      {openRevision}
    />
  {/if}
  <HistoryDetail {origin} {revision} {comparison} {rendered} />
</section>

<style>
  .history-controls {
    margin-block: 1rem;
  }
  fieldset {
    margin-block: 0.75rem;
    border: 0;
    padding: 0;
  }
  fieldset label {
    display: block;
  }
  fieldset input {
    margin-right: 0.4rem;
  }
  .history-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-block: 0.75rem;
  }
</style>
