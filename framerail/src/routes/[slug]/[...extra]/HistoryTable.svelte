<script lang="ts">
  import { resolve } from "$app/paths"
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

  const flagNames: Record<string, { label: string; title: string }> = {
    N: { label: "New", title: "Page created" },
    S: { label: "Source", title: "Source changed" },
    T: { label: "Title", title: "Title changed" },
    R: { label: "Renamed", title: "Page name changed" },
    A: { label: "Tags", title: "Tags changed" },
    M: { label: "Metadata", title: "Metadata changed" },
    F: { label: "Files", title: "Files changed" }
  }

  const dateFormat = new Intl.DateTimeFormat("en-GB", {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone: "UTC"
  })

  function authorLabel(row: HistoryRow): string | null {
    if (row.author_name) return row.author_name
    if (row.author_id === null) return null
    return listing.origin === "wikidot"
      ? `Wikidot ID ${row.author_id}`
      : `Local ID ${row.author_id}`
  }
</script>

<div class="history-summary">
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
</div>
{#if listing.rows.length}
  <div class="history-table">
    <table class="page-history">
      <thead>
        <tr>
          <th class="col-rev" scope="col">Rev.</th>
          <th class="col-compare" aria-label="Compare revisions" scope="col"
            ><span class="compare-from">From</span><span class="compare-to">To</span></th
          >
          <th scope="col">Changes</th>
          <th scope="col"><span class="visually-hidden">Actions</span></th>
          <th scope="col">By</th>
          <th scope="col">Date (UTC)</th>
          <th scope="col">Comments</th>
        </tr>
      </thead>
      <tbody>
        {#each listing.rows as row (row.id)}
          {@const author = authorLabel(row)}
          <tr class:is-from={from === row.number} class:is-to={to === row.number}>
            <td class="col-rev">{row.number}</td>
            <td class="select-versions">
              <input
                name="from"
                aria-label={`Compare from revision ${row.number}`}
                checked={from === row.number}
                onchange={() => selectFrom(row.number)}
                title="Compare from this revision"
                type="radio"
              />
              <input
                name="to"
                aria-label={`Compare to revision ${row.number}`}
                checked={to === row.number}
                onchange={() => selectTo(row.number)}
                title="Compare to this revision"
                type="radio"
              />
            </td>
            <td class="flags">
              {#each row.flags as flag (flag)}
                <span
                  class="flag flag-{flag}"
                  data-flag={flag}
                  title={flagNames[flag]?.title}>{flagNames[flag]?.label ?? flag}</span
                >
              {:else}
                <span class="muted">—</span>
              {/each}
            </td>
            <td class="actions">
              <button
                aria-label={`View revision ${row.number}`}
                disabled={busy}
                onclick={() => openRevision(row, true)}
                title="View page revision"
                type="button">View</button
              >
              <button
                aria-label={`View source of revision ${row.number}`}
                disabled={busy}
                onclick={() => openRevision(row, false)}
                title="View source of revision"
                type="button">Wikitext</button
              >
            </td>
            <td class="author">
              {#if author && row.author_slug}<a
                  href={resolve("/-/user/[slug]", { slug: row.author_slug })}>{author}</a
                >{:else if author}{author}{:else}<span class="muted">Unknown</span>{/if}
            </td>
            <td class="date">
              <time datetime={row.created_at}
                >{dateFormat.format(new Date(row.created_at))}</time
              >
            </td>
            <td class="comments">{row.comments}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{:else}
  <p class="history-empty">No revisions match this selection.</p>
{/if}

<style lang="scss">
  .history-summary {
    --history-line: color-mix(in srgb, currentColor 16%, transparent);
    --history-accent: #1f5fa8;

    display: flex;
    flex-wrap: wrap;
    gap: 8px 16px;
    align-items: center;
    justify-content: space-between;
    margin-block: 16px 10px;

    p {
      margin: 0;
      font-variant-numeric: tabular-nums;
    }
  }

  .history-pages {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;

    button {
      min-width: 32px;
      min-height: 32px;
      padding: 0 8px;
      font: inherit;
      font-size: 0.9em;
      font-variant-numeric: tabular-nums;
      color: var(--history-accent);
      cursor: pointer;
      background: transparent;
      border: 1px solid var(--history-line);
      border-radius: var(--size-border-radius, 6px);

      &:hover:not(:disabled) {
        background: color-mix(in srgb, var(--history-accent) 8%, transparent);
      }

      &[aria-current="page"] {
        color: #fff;
        background: var(--history-accent);
        border-color: var(--history-accent);
      }

      &:disabled:not([aria-current]) {
        cursor: progress;
        opacity: 0.5;
      }
    }
  }

  .history-table {
    --history-line: color-mix(in srgb, currentColor 16%, transparent);
    --history-tint: color-mix(in srgb, currentColor 4%, transparent);
    --history-muted: color-mix(in srgb, currentColor 60%, transparent);
    --history-accent: #1f5fa8;
    --history-to: #1d7a44;

    overflow-x: auto;
    border: 1px solid var(--history-line);
    border-radius: var(--size-border-radius, 6px);
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th,
  td {
    padding: 8px 10px;
    line-height: 1.4;
    vertical-align: middle;
    text-align: left;
  }

  th {
    font-size: 0.75em;
    font-weight: 600;
    color: var(--history-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    white-space: nowrap;
    background: var(--history-tint);
    border-bottom: 1px solid var(--history-line);
  }

  tbody tr {
    box-shadow: inset 3px 0 0 transparent;

    + tr {
      border-top: 1px solid var(--history-line);
    }

    &:hover {
      background: var(--history-tint);
    }

    // Selected comparison endpoints: blue edge for "from", green for "to".
    &.is-from {
      box-shadow: inset 3px 0 0 var(--history-accent);
    }

    &.is-to {
      box-shadow: inset 3px 0 0 var(--history-to);
    }
  }

  .col-rev {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  .col-compare span {
    display: inline-block;
    width: 36px;
    text-align: center;
    letter-spacing: 0;

    &.compare-from {
      color: var(--history-accent);
    }

    &.compare-to {
      color: var(--history-to);
    }
  }

  .select-versions {
    white-space: nowrap;

    input {
      width: 16px;
      height: 16px;
      margin: 0 10px;
      accent-color: var(--history-accent);
      cursor: pointer;

      &[name="to"] {
        accent-color: var(--history-to);
      }
    }
  }

  .flags {
    white-space: nowrap;
  }

  .flag {
    display: inline-block;
    padding: 1px 8px;
    margin-right: 4px;
    font-size: 0.8em;
    font-weight: 600;
    border: 1px solid var(--history-line);
    border-radius: 999px;

    &.flag-N {
      color: var(--history-to);
      border-color: currentColor;
    }

    &.flag-R,
    &.flag-T {
      color: var(--history-accent);
      border-color: currentColor;
    }
  }

  .actions {
    white-space: nowrap;

    button {
      min-height: 30px;
      padding: 2px 10px;
      margin-right: 4px;
      font: inherit;
      font-size: 0.85em;
      color: var(--history-accent);
      cursor: pointer;
      background: transparent;
      border: 1px solid var(--history-line);
      border-radius: var(--size-border-radius, 6px);

      &:hover:not(:disabled) {
        background: color-mix(in srgb, var(--history-accent) 8%, transparent);
        border-color: var(--history-accent);
      }

      &:focus-visible {
        outline: 2px solid var(--history-accent);
        outline-offset: 1px;
      }

      &:disabled {
        cursor: progress;
        opacity: 0.5;
      }
    }
  }

  .author {
    white-space: nowrap;
  }

  .author a {
    font-weight: 500;
  }

  .date {
    font-variant-numeric: tabular-nums;
    color: var(--history-muted);
    white-space: nowrap;
  }

  .comments {
    min-width: 12rem;
    font-size: 0.9em;
    color: var(--history-muted);
    overflow-wrap: anywhere;
  }

  .muted {
    color: var(--history-muted);
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    white-space: nowrap;
    clip-path: inset(50%);
  }

  .history-empty {
    padding: 24px 16px;
    color: var(--history-muted, inherit);
    text-align: center;
    border: 1px dashed color-mix(in srgb, currentColor 16%, transparent);
    border-radius: var(--size-border-radius, 6px);
  }
</style>
