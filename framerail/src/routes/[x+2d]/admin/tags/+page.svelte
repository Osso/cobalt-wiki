<script lang="ts">
  import { enhance } from "$app/forms"
  import { resolve } from "$app/paths"
  import type { SubmitFunction } from "@sveltejs/kit"
  import type {
    TagAddFilters,
    TagAddPreview,
    TagPreview
  } from "$lib/server/deepwell/tag-admin"
  import type { PageProps } from "./$types"

  type Pending = { form: string; label: string; startedAt: number }

  let { data, form }: PageProps = $props()
  let operation = $state("rename")
  let pending = $state<Pending | null>(null)
  let now = $state(0)
  let dismissed = $state(false)
  const preview = $derived(dismissed ? null : form?.preview)
  const elapsed = $derived(
    pending ? Math.max(0, Math.floor((now - pending.startedAt) / 1000)) : 0
  )

  $effect(() => {
    if (!pending) return
    const timer = setInterval(() => (now = Date.now()), 1000)
    return () => clearInterval(timer)
  })

  function applyLabel(snapshot: TagPreview | TagAddPreview) {
    if ("tag" in snapshot) return `Adding ${snapshot.tag} to ${snapshot.count} pages…`
    if (snapshot.target_tag === null) {
      return `Removing ${snapshot.source_tag} from ${snapshot.count} pages…`
    }
    return `Renaming ${snapshot.source_tag} to ${snapshot.target_tag} on ${snapshot.count} pages…`
  }

  function describeFilters(filters: TagAddFilters) {
    return [
      filters.existing_tag && `Existing tag: ${filters.existing_tag}`,
      filters.category && `Category: ${filters.category}`,
      filters.title_contains && `Title contains: ${filters.title_contains}`
    ]
      .filter(Boolean)
      .join(" · ")
  }

  function submitting(form: string, label: string): SubmitFunction {
    return () => {
      now = Date.now()
      pending = { form, label, startedAt: now }
      dismissed = false
      return async ({ update }) => {
        try {
          await update()
        } finally {
          pending = null
        }
      }
    }
  }
</script>

{#snippet progress(form: string)}
  {#if pending?.form === form}
    <p class="progress">
      <span role="status">{pending.label}</span>
      <span aria-hidden="true">{elapsed}s</span>
    </p>
  {/if}
{/snippet}

<h1>Manage tags</h1>
<p><a href={resolve("/-/admin", {})}>Site Manager</a></p>

{#if data.access === "signed-out"}
  <p>
    You need to <a href={resolve("/-/login", {})}>sign in</a> as an administrator of this site.
  </p>
{:else if data.access === "denied"}
  <p role="status">Only administrators of this site can manage tags.</p>
{:else}
  <p>
    Rename or remove a tag across current pages, or add a tag to matching pages. Page
    content and historical revisions are preserved.
  </p>
  {#if form?.message}
    <p class:error={!form.saved} role="status">{form.message}</p>
  {/if}

  <form
    action="?/preview"
    method="POST"
    use:enhance={submitting("preview", "Finding affected pages…")}
  >
    <fieldset disabled={pending !== null}>
      <legend>Choose a tag change</legend>
      <label for="tag-operation">Operation</label>
      <select id="tag-operation" name="operation" bind:value={operation}>
        <option value="rename">Rename / merge</option>
        <option value="delete">Delete tag</option>
        <option value="add">Add tag to matching pages</option>
      </select>
      {#if operation === "add"}
        <label for="add-tag">Tag to add</label>
        <input id="add-tag" name="tag" autocomplete="off" required />
        <p>
          Use at least one filter. Pages must match all selected filters. Pages already
          carrying this tag are excluded.
        </p>
        <label for="filter-tag">Has existing tag</label>
        <input id="filter-tag" name="existingTag" autocomplete="off" />
        <label for="filter-category">Category</label>
        <input id="filter-category" name="category" autocomplete="off" />
        <p>
          Use the category slug, such as writing, or _default for uncategorized pages.
        </p>
        <label for="filter-title">Title contains</label>
        <input id="filter-title" name="titleContains" autocomplete="off" />
        <p>Literal text, ignoring case. Titles only; page bodies are not searched.</p>
      {:else}
        <label for="source-tag">Existing tag</label>
        <input id="source-tag" name="sourceTag" autocomplete="off" required />
      {/if}
      {#if operation === "rename"}
        <label for="target-tag">Destination tag</label>
        <input id="target-tag" name="targetTag" autocomplete="off" required />
        <p>If a page already has the destination tag, it keeps one copy.</p>
      {/if}
      <button type="submit">Preview affected pages</button>
      {@render progress("preview")}
    </fieldset>
  </form>

  {#if preview}
    <section aria-labelledby="preview-heading">
      <h2 id="preview-heading">Preview: {preview.count} pages</h2>
      <p>
        {#if "tag" in preview}
          Add <strong>{preview.tag}</strong> to these pages. All selected filters must match.
        {:else if preview.target_tag === null}
          Remove <strong>{preview.source_tag}</strong> from these pages. Pages will not be deleted.
        {:else}
          Replace <strong>{preview.source_tag}</strong> with
          <strong>{preview.target_tag}</strong> on these pages.
        {/if}
      </p>
      {#if "tag" in preview}
        <p>{describeFilters(preview)}</p>
      {/if}
      <ul class="page-list">
        {#each preview.pages as page (page.page_id)}
          <li><a href={`/${page.slug}`}>{page.title}</a> <small>({page.slug})</small></li>
        {:else}
          <li>No pages need this tag change.</li>
        {/each}
      </ul>
      {#if preview.count > 0}
        <form
          action="?/apply"
          method="POST"
          use:enhance={submitting("apply", applyLabel(preview))}
        >
          {#if "tag" in preview}
            <input name="operation" type="hidden" value="add" />
            <input name="tag" type="hidden" value={preview.tag} />
            <input name="existingTag" type="hidden" value={preview.existing_tag ?? ""} />
            <input name="category" type="hidden" value={preview.category ?? ""} />
            <input
              name="titleContains"
              type="hidden"
              value={preview.title_contains ?? ""}
            />
          {:else}
            <input name="sourceTag" type="hidden" value={preview.source_tag} />
            <input name="targetTag" type="hidden" value={preview.target_tag ?? ""} />
            <input
              name="operation"
              type="hidden"
              value={preview.target_tag === null ? "delete" : "rename"}
            />
          {/if}
          <input name="previewToken" type="hidden" value={preview.preview_token} />
          <p>Confirm this exact preview. If pages changed, a new preview is required.</p>
          <div class="actions">
            <button
              name="confirmed"
              disabled={pending !== null}
              type="submit"
              value="yes"
            >
              {#if "tag" in preview}Confirm add tag{:else if preview.target_tag === null}Confirm
                delete tag{:else}Confirm rename{/if}
            </button>
            <button
              disabled={pending !== null}
              onclick={() => (dismissed = true)}
              type="button">Cancel</button
            >
          </div>
          {@render progress("apply")}
        </form>
      {/if}
    </section>
  {/if}

  <section aria-labelledby="history-heading">
    <h2 id="history-heading">Tag operation history</h2>
    <p>
      Undo creates new revisions. If any affected page was edited afterward, undo refuses
      the entire operation to preserve later edits.
    </p>
    {#each data.operations as item (item.operation_id)}
      <article>
        <h3>
          #{item.operation_id}: {#if item.kind === "add"}Add {item.target_tag}{:else}{item.source_tag}
            → {item.target_tag ?? "deleted"}{/if}
        </h3>
        {#if item.filters}<p>{describeFilters(item.filters)}</p>{/if}
        <p>
          {item.count} pages · User #{item.actor_user_id} ·
          <time datetime={item.created_at}>{item.created_at}</time>
        </p>
        <details>
          <summary>Affected pages</summary>
          <ul class="page-list">
            {#each item.pages as page (page.page_id)}
              <li>
                <a href={`/${page.slug}`}>{page.title}</a> <small>({page.slug})</small>
              </li>
            {/each}
          </ul>
        </details>
        {#if item.reversed}
          <p>Undone</p>
        {:else}
          <details>
            <summary>Undo this operation</summary>
            <form
              action="?/undo"
              method="POST"
              use:enhance={submitting(
                `undo-${item.operation_id}`,
                `Undoing #${item.operation_id} on ${item.count} pages…`
              )}
            >
              <input name="operationId" type="hidden" value={item.operation_id} />
              <p>
                Restore the prior tags on all {item.count} pages from operation #{item.operation_id}?
              </p>
              <button
                name="confirmed"
                disabled={pending !== null}
                type="submit"
                value="yes">Confirm undo #{item.operation_id}</button
              >
              {@render progress(`undo-${item.operation_id}`)}
            </form>
          </details>
        {/if}
      </article>
    {:else}
      <p>No site-wide tag operations yet.</p>
    {/each}
  </section>
{/if}

<style lang="scss">
  fieldset {
    display: grid;
    gap: 0.75rem;
    max-width: 36rem;
    padding: 1rem;
  }

  input,
  select,
  button,
  summary {
    min-height: 44px;
    font: inherit;
  }

  input,
  select {
    box-sizing: border-box;
    width: 100%;
    min-width: 0;
  }

  button,
  summary {
    cursor: pointer;
  }

  button:disabled {
    cursor: wait;
  }

  section,
  article {
    margin-top: 1.5rem;
  }

  article {
    padding-bottom: 1rem;
    border-bottom: 1px solid currentcolor;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  .page-list {
    max-height: 24rem;
    overflow-y: auto;
    overflow-wrap: anywhere;
  }

  .error {
    font-weight: bold;
  }

  .progress {
    display: flex;
    gap: 0.5rem;
    align-items: center;

    &::before {
      width: 1em;
      height: 1em;
      content: "";
      border: 2px solid currentcolor;
      border-right-color: transparent;
      border-radius: 50%;
      animation: spin 0.8s linear infinite;
    }
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .progress::before {
      animation: none;
    }
  }

  :is(input, select, button, summary):focus-visible {
    outline: 2px solid currentcolor;
    outline-offset: 3px;
  }
</style>
