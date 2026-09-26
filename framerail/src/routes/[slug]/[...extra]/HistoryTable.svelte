<script lang="ts">
  import type { HistoryList, HistoryRow } from "$lib/server/load/history"

  let {
    listing,
    from,
    to,
    busy,
    selectFrom,
    selectTo,
    changePage,
    openRevision
  }: {
    listing: HistoryList
    from: number | null
    to: number | null
    busy: boolean
    selectFrom: (number: number) => void
    selectTo: (number: number) => void
    changePage: (page: number) => void
    openRevision: (row: HistoryRow, rendered: boolean) => void
  } = $props()
</script>

<p>{listing.total} revisions · Page {listing.page} of {listing.total_pages}</p>
{#if listing.total_pages > 1}
  <nav aria-label="History pages" class="history-pages">
    {#each Array.from({ length: listing.total_pages }, (_, index) => index + 1) as pageNumber}
      <button
        type="button"
        aria-label={`Go to page ${pageNumber}`}
        aria-current={pageNumber === listing.page ? "page" : undefined}
        disabled={busy || pageNumber === listing.page}
        onclick={() => changePage(pageNumber)}>{pageNumber}</button
      >
    {/each}
  </nav>
{/if}
{#if listing.rows.length}
  <div class="history-table">
    <table class="page-history">
      <thead
        ><tr>
          <th scope="col">rev.</th><th scope="col" aria-label="Compare revisions"></th><th
            scope="col">flags</th
          >
          <th scope="col">actions</th><th scope="col">by</th><th scope="col">date</th>
          <th scope="col">comments</th>
        </tr></thead
      >
      <tbody>
        {#each listing.rows as row (row.id)}
          <tr>
            <td>{row.number}.</td>
            <td class="select-versions">
              <label
                ><input
                  type="radio"
                  name="from"
                  checked={from === row.number}
                  onchange={() => selectFrom(row.number)}
                  aria-label={`Compare from revision ${row.number}`}
                /></label
              >
              <label
                ><input
                  type="radio"
                  name="to"
                  checked={to === row.number}
                  onchange={() => selectTo(row.number)}
                  aria-label={`Compare to revision ${row.number}`}
                /></label
              >
            </td>
            <td>{row.flags.join(" ") || "—"}</td>
            <td class="actions">
              <button
                type="button"
                title="View page revision"
                aria-label={`View revision ${row.number}`}
                disabled={busy}
                onclick={() => openRevision(row, true)}>V</button
              >
              <button
                type="button"
                title="View source of revision"
                aria-label={`View source of revision ${row.number}`}
                disabled={busy}
                onclick={() => openRevision(row, false)}>S</button
              >
            </td>
            <td
              >{row.author_name ??
                (listing.origin === "wikidot" && row.author_id !== null
                  ? `Wikidot ID ${row.author_id}`
                  : listing.origin === "local" && row.author_id !== null
                    ? `Local ID ${row.author_id}`
                    : "Unknown")}</td
            >
            <td
              ><time datetime={row.created_at}
                >{new Intl.DateTimeFormat("en-US", {
                  year: "numeric",
                  month: "short",
                  day: "numeric",
                  timeZone: "UTC"
                }).format(new Date(row.created_at))}</time
              ></td
            >
            <td>{row.comments}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{:else}
  <p>No revisions match this selection.</p>
{/if}

<style>
  .history-table {
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  td,
  th {
    padding: 0.25rem 0.4rem;
    text-align: left;
    vertical-align: top;
  }
  .select-versions,
  .actions,
  .history-pages {
    white-space: nowrap;
  }
  .select-versions label {
    display: inline-flex;
    align-items: center;
    margin-right: 0.35rem;
  }
  .actions button {
    padding: 0;
    border: 0;
    background: none;
    color: var(--link-color, #06c);
    text-decoration: underline;
    cursor: pointer;
  }
  .actions button:disabled {
    cursor: default;
  }
  .history-pages button {
    min-width: 2rem;
  }
  .history-pages {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
    margin-block: 1rem;
  }
</style>
