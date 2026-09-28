<script lang="ts">
  import { enhance } from "$app/forms"
  import { resolve } from "$app/paths"
  import type { SubmitFunction } from "@sveltejs/kit"
  import type { PageProps } from "./$types"

  let { data, form }: PageProps = $props()
  let operation = $state("rename")
  let pending = $state(false)
  let dismissed = $state(false)
  const preview = $derived(dismissed ? null : form?.preview)
  const submit: SubmitFunction = () => {
    pending = true
    dismissed = false
    return async ({ update }) => {
      try {
        await update()
      } finally {
        pending = false
      }
    }
  }
</script>

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
    Rename or remove a tag across all current pages. Page content and historical revisions
    are preserved.
  </p>
  {#if form?.message}
    <p role="status" class:error={!form.saved}>{form.message}</p>
  {/if}
  {#if pending}<p role="status">Working…</p>{/if}

  <form action="?/preview" method="POST" use:enhance={submit}>
    <fieldset disabled={pending}>
      <legend>Choose a tag change</legend>
      <label for="tag-operation">Operation</label>
      <select id="tag-operation" name="operation" bind:value={operation}>
        <option value="rename">Rename / merge</option>
        <option value="delete">Delete tag</option>
      </select>
      <label for="source-tag">Existing tag</label>
      <input id="source-tag" name="sourceTag" required autocomplete="off" />
      {#if operation === "rename"}
        <label for="target-tag">Destination tag</label>
        <input id="target-tag" name="targetTag" required autocomplete="off" />
        <p>If a page already has the destination tag, it keeps one copy.</p>
      {/if}
      <button type="submit">Preview affected pages</button>
    </fieldset>
  </form>

  {#if preview}
    <section aria-labelledby="preview-heading">
      <h2 id="preview-heading">Preview: {preview.count} pages</h2>
      <p>
        {#if preview.target_tag === null}
          Remove <strong>{preview.source_tag}</strong> from these pages. Pages will not be deleted.
        {:else}
          Replace <strong>{preview.source_tag}</strong> with
          <strong>{preview.target_tag}</strong> on these pages.
        {/if}
      </p>
      <ul class="page-list">
        {#each preview.pages as page (page.page_id)}
          <li><a href={"/" + page.slug}>{page.title}</a> <small>({page.slug})</small></li>
        {:else}
          <li>No current pages have this tag.</li>
        {/each}
      </ul>
      {#if preview.count > 0}
        <form action="?/apply" method="POST" use:enhance={submit}>
          <input type="hidden" name="sourceTag" value={preview.source_tag} />
          <input type="hidden" name="targetTag" value={preview.target_tag ?? ""} />
          <input
            type="hidden"
            name="operation"
            value={preview.target_tag === null ? "delete" : "rename"}
          />
          <input type="hidden" name="previewToken" value={preview.preview_token} />
          <p>Confirm this exact preview. If pages changed, a new preview is required.</p>
          <div class="actions">
            <button type="submit" name="confirmed" value="yes" disabled={pending}>
              {preview.target_tag === null ? "Confirm delete tag" : "Confirm rename"}
            </button>
            <button type="button" disabled={pending} onclick={() => (dismissed = true)}
              >Cancel</button
            >
          </div>
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
        <h3>#{item.operation_id}: {item.source_tag} → {item.target_tag ?? "deleted"}</h3>
        <p>
          {item.count} pages · User #{item.actor_user_id} ·
          <time datetime={item.created_at}>{item.created_at}</time>
        </p>
        <details>
          <summary>Affected pages</summary>
          <ul class="page-list">
            {#each item.pages as page (page.page_id)}
              <li>
                <a href={"/" + page.slug}>{page.title}</a> <small>({page.slug})</small>
              </li>
            {/each}
          </ul>
        </details>
        {#if item.reversed}
          <p>Undone</p>
        {:else}
          <details>
            <summary>Undo this operation</summary>
            <form action="?/undo" method="POST" use:enhance={submit}>
              <input type="hidden" name="operationId" value={item.operation_id} />
              <p>
                Restore the prior tags on all {item.count} pages from operation #{item.operation_id}?
              </p>
              <button type="submit" name="confirmed" value="yes" disabled={pending}
                >Confirm undo #{item.operation_id}</button
              >
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
    min-width: 0;
    width: 100%;
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

  :is(input, select, button, summary):focus-visible {
    outline: 2px solid currentcolor;
    outline-offset: 3px;
  }
</style>
