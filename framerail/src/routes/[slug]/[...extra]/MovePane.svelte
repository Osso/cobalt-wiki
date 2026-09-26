<script lang="ts">
  import { deserialize } from "$app/forms"
  import { goto } from "$app/navigation"
  import { errorPopupState, pageLayoutState } from "$lib/stores.svelte"
  import { Layout, PagePane, ToastType } from "$lib/types"
  import { toast } from "$lib/component/scripts/toasts"
  import { resolve } from "$app/paths"
  import { superForm } from "sveltekit-superforms"
  import { untrack } from "svelte"

  import type { PageBacklinks } from "$lib/server/load/page-backlinks"
  import MoveDependencies from "./MoveDependencies.svelte"
  import type { PageProps } from "./$types"

  let { pagePaneState = $bindable(), data }: PageProps & { pagePaneState: PagePane } =
    $props()

  let dependencies = $state<PageBacklinks | null>(null)
  let remaining = $state<PageBacklinks | null>(null)
  let selectedIds = $state<number[]>([])
  let movedSlug = $state("")
  let loading = $state(false)
  let loadError = $state("")
  let requestId = 0

  function closePane() {
    requestId++
    pagePaneState = PagePane.None
  }

  async function showDependencies() {
    const currentRequest = ++requestId
    loading = true
    loadError = ""
    try {
      const response = await fetch("?/backlinks", {
        method: "POST",
        headers: { "content-type": "application/x-www-form-urlencoded" },
        body: ""
      })
      const result = deserialize<{ res: PageBacklinks }, { message?: string }>(
        await response.text()
      )
      if (result.type !== "success" || !result.data) {
        throw new Error(
          result.type === "failure"
            ? (result.data?.message ?? "Unable to load dependencies")
            : "Unable to load dependencies"
        )
      }
      if (currentRequest === requestId) dependencies = result.data.res
    } catch (cause) {
      if (currentRequest === requestId) {
        loadError = cause instanceof Error ? cause.message : "Unable to load dependencies"
      }
    } finally {
      if (currentRequest === requestId) loading = false
    }
  }

  const { form, enhance } = superForm(
    untrack(() => data.forms.pageMoveForm),
    {
      dataType: "json",
      onSubmit: async ({ jsonData }) => {
        const submitForm = {
          ...$form,
          fixDependencies: [...selectedIds],
          siteId: data.site.site_id,
          pageId: data.page?.page_id,
          lastRevisionId: data.page_revision?.revision_id
        }
        jsonData(submitForm)
      },
      onResult: async ({ result, cancel }) => {
        if (result.type === "success" && result.data) {
          cancel()
          toast(ToastType.Success, data.internationalization!["wiki-page-move.toast"]!)
          const moved = result.data.res
          movedSlug = moved.new_slug
          const leftovers = moved.remaining_dependencies
          if (leftovers.links.length || leftovers.inclusions.length) {
            remaining = leftovers
          } else {
            goto(resolve(`/${movedSlug}`, {}), { noScroll: true })
            closePane()
          }
        }
        if (result.type === "failure" && result.data) {
          errorPopupState.current = {
            state: true,
            message: result.data.message,
            data: result.data.data
          }
        }
      }
    }
  )
</script>

{#if pageLayoutState.current === Layout.WIKIDOT}
  <h1 class="page-move-header">
    {data.internationalization?.["wiki-page-move"]}
  </h1>
{:else}
  <h2 class="page-move-header">
    {data.internationalization?.["wiki-page-move"]}
  </h2>
{/if}

{#if remaining}
  <div class="page-move">
    <p>Page moved, but some dependencies remain.</p>
    <MoveDependencies dependencies={remaining} selectedIds={[]} remaining />
    <button
      type="button"
      onclick={() => goto(resolve(`/${movedSlug}`, {}))}
    >
      Continue to new page
    </button>
  </div>
{:else}
  <form id="page-move" class="page-move" action="?/move" method="POST" use:enhance>
    <input
      name="new-slug"
      class="page-move-new-slug"
      placeholder={data.internationalization?.["wiki-page-move.new-slug"]}
      type="text"
      bind:value={$form.newSlug}
    />
    <textarea
      name="comments"
      class="page-move-comments"
      placeholder={data.internationalization?.["wiki-page-revision-comments"]}
      bind:value={$form.comments}></textarea>
    {#if dependencies}
      <button
        type="button"
        onclick={() => {
          requestId++
          dependencies = null
          selectedIds = []
        }}>Hide dependencies</button
      >
      <MoveDependencies {dependencies} bind:selectedIds />
    {:else}
      <button type="button" disabled={loading} onclick={showDependencies}>
        {loading ? "Loading dependencies…" : "Show dependencies"}
      </button>
    {/if}
    {#if loadError}<p role="alert">{loadError}</p>{/if}
    {#if pageLayoutState.current === Layout.WIKIDOT}
      <div class="buttons">
        <input
          class="btn btn-danger"
          onclick={closePane}
          type="button"
          value={data.internationalization?.cancel}
        />
        <input
          class="btn btn-primary"
          type="submit"
          value={data.internationalization?.move}
        />
      </div>
    {:else}
      <div class="action-row page-move-actions">
        <button
          class="action-button page-move-button button-cancel clickable"
          onclick={closePane}
          type="button"
        >
          {data.internationalization?.cancel}
        </button>
        <button
          class="action-button page-move-button button-move clickable"
          type="submit"
        >
          {data.internationalization?.move}
        </button>
      </div>
    {/if}
  </form>
{/if}

<style lang="scss">
  .page-move {
    display: flex;
    flex-direction: column;
    gap: 15px;
    align-items: stretch;
    justify-content: stretch;
    width: 100%;
    padding: 0 0 2em;
  }
</style>
