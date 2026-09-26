<script lang="ts" module>
  import { deserialize } from "$app/forms"

  import type { PageBlockState } from "$lib/server/deepwell/page"

  type BlockFetch = (
    url: string,
    init: RequestInit
  ) => Promise<{ text(): Promise<string> }>

  async function blockRequest<T>(
    fetcher: BlockFetch,
    action: "blockGet" | "blockSet",
    body: { pageId: number; blocked?: boolean }
  ): Promise<T> {
    const response = await fetcher(`?/${action}`, {
      method: "POST",
      body: JSON.stringify(body)
    })
    const result = deserialize<{ res: T }, { message?: string }>(await response.text())
    if (result.type !== "success") {
      throw new Error(
        result.type === "failure"
          ? (result.data?.message ?? "Unable to update page Block state")
          : "Unable to update page Block state"
      )
    }
    if (action === "blockGet" && !result.data?.res) {
      throw new Error("Unable to load page Block state")
    }
    return result.data?.res as T
  }

  export function loadBlock(
    fetcher: BlockFetch,
    pageId: number
  ): Promise<PageBlockState> {
    return blockRequest(fetcher, "blockGet", { pageId })
  }

  export function saveBlock(
    fetcher: BlockFetch,
    pageId: number,
    blocked: boolean
  ): Promise<void> {
    return blockRequest(fetcher, "blockSet", { pageId, blocked })
  }
</script>

<script lang="ts">
  import { PagePane } from "$lib/types"
  import { errorPopupState } from "$lib/stores.svelte"

  import type { PageProps } from "./$types"

  let { pagePaneState = $bindable(), data }: PageProps & { pagePaneState: PagePane } =
    $props()

  let blocked = $state(false)
  let canManage = $state(false)
  let loaded = $state(false)
  let saving = $state(false)
  let message = $state("")

  $effect(() => {
    let current = true
    loadBlock(fetch, data.page!.page_id)
      .then((state) => {
        if (!current) return
        blocked = state.blocked
        canManage = state.can_manage
        loaded = true
      })
      .catch((error: unknown) => {
        if (!current) return
        message =
          error instanceof Error ? error.message : "Unable to load page Block state"
        errorPopupState.current = { state: true, message, data: null }
      })
    return () => {
      current = false
    }
  })

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    if (!loaded || !canManage || saving) return
    saving = true
    message = ""
    try {
      await saveBlock(fetch, data.page!.page_id, blocked)
      pagePaneState = PagePane.None
    } catch (error) {
      message =
        error instanceof Error ? error.message : "Unable to update page Block state"
      errorPopupState.current = { state: true, message, data: null }
    } finally {
      saving = false
    }
  }
</script>

<h1>Block this page</h1>
<p>
  When the page is blocked, only Site Administrators and Moderators with enough privileges
  can edit or modify it.
</p>
<form onsubmit={submit}>
  <label for="page-block-checkbox">
    Page blocked
    <input
      id="page-block-checkbox"
      type="checkbox"
      bind:checked={blocked}
      disabled={!loaded || !canManage || saving}
    />
  </label>
  {#if !loaded && !message}
    <p role="status">Loading page Block state…</p>
  {:else if loaded && !canManage}
    <p role="status">
      Only Site Administrators and authorized Moderators can manage page blocks.
    </p>
  {/if}
  {#if message}<p role="alert">{message}</p>{/if}
  <div class="buttons">
    <button
      type="button"
      class="btn btn-default"
      onclick={() => (pagePaneState = PagePane.None)}>Cancel</button
    >
    <button
      type="submit"
      class="btn btn-primary"
      disabled={!loaded || !canManage || saving}
    >
      {saving ? "Saving…" : "Save"}
    </button>
  </div>
</form>
