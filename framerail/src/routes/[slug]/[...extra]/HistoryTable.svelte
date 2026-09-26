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
  <nav class="history-pages" aria-label="History pages">
    {#each Array.from({ length: listing.total_pages }, (_, index) => index + 1) as pageNumber (pageNumber)}
      <button
        aria-current={pageNumber === listing.page ? "page" : undefined}
        aria-label={`Go to page ${pageNumber}`}
        disabled={busy || pageNumber === listing.page}
        onclick={() => changePage(pageNumber)}
        type="button">{pageNumber}</button
      >
    {/each}
  </nav>
{/if}
{#if listing.rows.length}
  <div class="history-table">
    <table class="page-history">
      <thead
        ><tr>
          <th scope="col">rev.</th><th aria-label="Compare revisions" scope="col"></th><th
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
                  name="from"
                  aria-label={`Compare from revision ${row.number}`}
                  checked={from === row.number}
                  onchange={() => selectFrom(row.number)}
                  type="radio"
                /></label
              >
              <label
                ><input
                  name="to"
                  aria-label={`Compare to revision ${row.number}`}
                  checked={to === row.number}
                  onchange={() => selectTo(row.number)}
                  type="radio"
                /></label
              >
            </td>
            <td>{row.flags.join(" ") || "—"}</td>
            <td class="actions">
              <button
                aria-label={`View revision ${row.number}`}
                disabled={busy}
                onclick={() => openRevision(row, true)}
                title="View page revision"
                type="button">V</button
              >
              <button
                aria-label={`View source of revision ${row.number}`}
                disabled={busy}
                onclick={() => openRevision(row, false)}
                title="View source of revision"
                type="button">S</button
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
                >{new Intl.DateTimeFormat("en-GB", {
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
    padding: 0.15rem 0.35rem;
    line-height: 1.35;
    vertical-align: top;
    text-align: left;
  }
  th {
    font-weight: normal;
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
    padding: 0.1rem 0.35rem;
    color: var(--link-color, #06c);
    cursor: pointer;
    background: #eee;
    border: 1px solid #aaa;
  }
  .actions button:disabled {
    cursor: default;
  }
  .history-pages button {
    min-width: 1.8rem;
    padding: 0.15rem 0.4rem;
    color: var(--link-color, #06c);
    background: #eee;
    border: 1px solid #aaa;
  }
  .history-pages button + button {
    border-left: 0;
  }
  .history-pages button[aria-current="page"] {
    color: inherit;
    background: #ccc;
  }
  .history-pages {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    margin-block: 1rem;
  }
</style>
