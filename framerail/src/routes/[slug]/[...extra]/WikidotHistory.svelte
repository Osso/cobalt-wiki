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
    <div class="history-toolbar">
      <div class="history-control">
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
          <span class="history-control-label">History dataset</span>
          <span class="history-control-value"
            >{origin === "wikidot" ? "Wikidot source" : "Local revisions"}</span
          >
        {/if}
      </div>
      <div class="history-control history-page-size">
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
          class="is-primary"
          disabled={busy || from === null || to === null || from === to}
          onclick={compareVersions}
          title="Pick a From and a To revision in the table first"
          type="button">Compare versions</button
        >
      </div>
    </div>
    <fieldset>
      <legend>Show page changes</legend>
      {#each filterLabels as filter (filter.key)}
        <label class="history-filter" class:is-checked={filters[filter.key]}
          ><input
            checked={filters[filter.key]}
            onchange={(event) => updateFilter(filter.key, event.currentTarget.checked)}
            type="checkbox"
          />{filter.label}</label
        >
      {/each}
    </fieldset>
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

<style lang="scss">
  .history-controls {
    --history-line: color-mix(in srgb, currentColor 16%, transparent);
    --history-tint: color-mix(in srgb, currentColor 4%, transparent);
    --history-muted: color-mix(in srgb, currentColor 60%, transparent);
    --history-accent: #1f5fa8;
    --history-radius: var(--size-border-radius, 6px);

    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 16px;
    margin-block: 16px;
    background: var(--history-tint);
    border: 1px solid var(--history-line);
    border-radius: var(--history-radius);
  }

  .history-toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 12px 20px;
    align-items: flex-end;
  }

  .history-control {
    display: flex;
    flex-direction: column;
    gap: 4px;

    label,
    .history-control-label {
      font-size: 0.8em;
      font-weight: 600;
      color: var(--history-muted);
    }

    .history-control-value {
      min-height: 34px;
      line-height: 34px;
    }

    select {
      min-height: 34px;
      padding: 4px 8px;
      font: inherit;
      background: transparent;
      border: 1px solid var(--history-line);
      border-radius: var(--history-radius);

      &:focus-visible {
        outline: 2px solid var(--history-accent);
        outline-offset: 1px;
      }
    }
  }

  .history-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-left: auto;

    button {
      min-height: 36px;
      padding: 6px 16px;
      font: inherit;
      font-weight: 600;
      color: inherit;
      cursor: pointer;
      background: transparent;
      border: 1px solid var(--history-line);
      border-radius: var(--history-radius);

      &:hover:not(:disabled) {
        border-color: var(--history-accent);
      }

      &.is-primary {
        color: #fff;
        background: var(--history-accent);
        border-color: var(--history-accent);

        &:hover:not(:disabled) {
          background: color-mix(in srgb, var(--history-accent) 85%, #000);
        }
      }

      &:focus-visible {
        outline: 3px solid color-mix(in srgb, var(--history-accent) 55%, transparent);
        outline-offset: 2px;
      }

      &:disabled {
        cursor: not-allowed;
        opacity: 0.5;
      }
    }
  }

  // Change filters as toggle pills; the native checkbox stays visible inside.
  fieldset {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    padding: 12px 0 0;
    margin: 0;
    border: 0;
    border-top: 1px solid var(--history-line);

    legend {
      float: left;
      margin-right: 6px;
      font-size: 0.8em;
      font-weight: 600;
      color: var(--history-muted);
    }
  }

  .history-filter {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    min-height: 32px;
    padding: 2px 12px 2px 8px;
    margin: 0;
    font-size: 0.9em;
    cursor: pointer;
    background: transparent;
    border: 1px solid var(--history-line);
    border-radius: 999px;

    input {
      margin: 0;
      accent-color: var(--history-accent);
    }

    &.is-checked {
      color: var(--history-accent);
      background: color-mix(in srgb, var(--history-accent) 8%, transparent);
      border-color: var(--history-accent);
    }

    &:has(:focus-visible) {
      outline: 2px solid var(--history-accent);
      outline-offset: 1px;
    }
  }
</style>
