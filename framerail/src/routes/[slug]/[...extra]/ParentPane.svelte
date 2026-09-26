<script module lang="ts">
  export function parentLookupQuery(input: string): string {
    const token = input.slice(input.lastIndexOf(" ") + 1)
    return token.length >= 2 ? token : ""
  }

  export function parentSuggestionValue(input: string, slug: string): string {
    return input.slice(0, input.lastIndexOf(" ") + 1) + slug
  }

  export async function fetchParentSuggestions(
    query: string,
    lookup: (query: string) => Promise<{ slug: string; title: string }[]>,
    isCurrent: () => boolean
  ): Promise<{ matches: { slug: string; title: string }[]; error: string } | undefined> {
    try {
      const matches = await lookup(query)
      return isCurrent() ? { matches, error: "" } : undefined
    } catch {
      return isCurrent()
        ? {
            matches: [],
            error: "Unable to look up pages. You can still enter page names."
          }
        : undefined
    }
  }
</script>

<script lang="ts">
  import { deserialize } from "$app/forms"
  import { invalidateAll } from "$app/navigation"
  import { errorPopupState, pageLayoutState } from "$lib/stores.svelte"
  import { Layout, PagePane, ToastType } from "$lib/types"
  import { toast } from "$lib/component/scripts/toasts"
  import { lookupEditorPages } from "$lib/editor-lookup"
  import { superForm } from "sveltekit-superforms"
  import { untrack } from "svelte"

  import type { PageProps } from "./$types"

  let pageParents = $state<string>("")
  let parentMatches = $state<{ slug: string; title: string }[]>([])
  let parentLookupError = $state("")

  let { pagePaneState = $bindable(), data }: PageProps & { pagePaneState: PagePane } =
    $props()

  const { form, enhance } = superForm(
    untrack(() => data.forms.pageParentForm),
    {
      dataType: "json",
      onSubmit: async ({ jsonData }) => {
        const { parents: formParents } = $form
        const newParents = formParents.split(" ").filter((p) => p)
        const oldParents = pageParents.split(" ").filter((p) => p)
        const removed: string[] = oldParents.filter((p) => !newParents.includes(p))
        const common: string[] = oldParents.filter((p) => newParents.includes(p))
        const added: string[] = newParents.filter((p) => !common.includes(p))

        const submitForm = {
          siteId: data.site.site_id,
          pageId: data.page?.page_id,
          addParents: added.length ? added : undefined,
          removeParents: removed.length ? removed : undefined
        }
        jsonData(submitForm)
      },
      onResult: async ({ result, cancel }) => {
        if (result.type === "success" && result.data) {
          toast(ToastType.Success, data.internationalization!["wiki-page-parent.toast"]!)
          cancel()
          pagePaneState = PagePane.None
          invalidateAll()
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

  async function fetchParents() {
    const res = await fetch(`?/parentGet`, {
      method: "POST",
      body: JSON.stringify({
        siteId: data.site.site_id,
        pageId: data.page?.page_id,
        slug: data.page?.slug
      })
    }).then((res) => res.text())

    const result = deserialize<
      { res: string[] },
      { message: string; code: string; data: Record<string, unknown> }
    >(res)

    if (result.type === "failure" && result.data?.message) {
      errorPopupState.current = {
        state: true,
        message: result.data.message,
        data: result.data.data
      }
    } else if (result.type === "success" && result.data?.res) {
      pageParents = result.data.res.join(" ")
      $form.parents = pageParents
    }
  }

  $effect(() => {
    fetchParents()
  })

  $effect(() => {
    if (pageLayoutState.current !== Layout.WIKIDOT) return
    const query = parentLookupQuery($form.parents)
    parentMatches = []
    parentLookupError = ""
    if (!query) return

    let current = true
    const timer = setTimeout(async () => {
      const result = await fetchParentSuggestions(query, lookupEditorPages, () => current)
      if (result) {
        parentMatches = result.matches
        parentLookupError = result.error
      }
    }, 500)
    return () => {
      current = false
      clearTimeout(timer)
    }
  })
</script>

{#if pageLayoutState.current === Layout.WIKIDOT}
  <h1 class="page-parent-header">
    {data.internationalization?.["wiki-page-parent"]}
  </h1>
{:else}
  <h2 class="page-parent-header">
    {data.internationalization?.["wiki-page-parent"]}
  </h2>
{/if}

<form id="page-parent" class="page-parent" action="?/parentSet" method="POST" use:enhance>
  {#if pageLayoutState.current === Layout.WIKIDOT}
    <p>
      Parent pages organize breadcrumbs for this page. Enter multiple parent page names as
      a space-separated list. Leave the field blank to remove all parents when you save.
    </p>
    <label for="parent-page-names">Parent page names</label>
  {/if}
  <input
    class="page-parent-new-parents"
    id={pageLayoutState.current === Layout.WIKIDOT ? "parent-page-names" : undefined}
    list={pageLayoutState.current === Layout.WIKIDOT
      ? "parent-page-suggestions"
      : undefined}
    autocomplete="off"
    placeholder={data.internationalization?.parents}
    type="text"
    bind:value={$form.parents}
  />
  {#if pageLayoutState.current === Layout.WIKIDOT}
    <datalist id="parent-page-suggestions">
      {#each parentMatches as match (match.slug)}
        <option value={parentSuggestionValue($form.parents, match.slug)}
          >{match.title}</option
        >
      {/each}
    </datalist>
    {#if parentLookupError}<p role="alert">{parentLookupError}</p>{/if}
  {/if}
  {#if pageLayoutState.current === Layout.WIKIDOT}
    <div class="buttons">
      <input
        class="btn btn-secondary"
        onclick={() => ($form.parents = "")}
        type="button"
        value="Clear parents"
      />
      <input
        class="btn btn-danger"
        onclick={() => (pagePaneState = PagePane.None)}
        type="button"
        value={data.internationalization?.cancel}
      />
      <input
        class="btn btn-primary"
        type="submit"
        value={data.internationalization?.save}
      />
    </div>
  {:else}
    <div class="action-row page-parent-actions">
      <button
        class="action-button page-parent-button button-cancel clickable"
        onclick={() => (pagePaneState = PagePane.None)}
        type="button"
      >
        {data.internationalization?.cancel}
      </button>
      <button
        class="action-button page-parent-button button-save clickable"
        type="submit"
      >
        {data.internationalization?.save}
      </button>
    </div>
  {/if}
</form>

<style lang="scss">
  .page-parent {
    display: flex;
    flex-direction: column;
    gap: 15px;
    align-items: stretch;
    justify-content: stretch;
    width: 100%;
    padding: 0 0 2em;
  }
</style>
